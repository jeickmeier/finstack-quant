//! Typed wrapper for return-contribution attribution results plus the
//! spec extractor shared with the entry point.

use crate::bindings::date_utils::extract_date_iso;
use crate::bindings::module_utils::py_to_json_value;
use crate::bindings::pandas_utils::{
    labeled_values_to_series, serde_rows_to_dataframe_with_schema, serde_to_py, ColumnSchema,
};
use crate::errors::{display_to_py, serde_json_to_py, value_error};
use finstack_quant_attribution::{ReturnContributionPosition, ReturnContributionSpec};
use pyo3::exceptions::PyTypeError;
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyDict};
use std::collections::BTreeMap;

/// Column schema of `ReturnContributionResult.to_dataframe`.
const INSTRUMENT_COLUMNS: &[ColumnSchema<'static>] = &[
    ("id", "str"),
    ("weight", "float64"),
    ("return", "float64"),
    ("contribution", "float64"),
    ("active_contribution", "float64"),
];

/// Column schema of `ReturnContributionResult.to_group_dataframe`.
const GROUP_COLUMNS: &[ColumnSchema<'static>] = &[
    ("dimension", "str"),
    ("key", "str"),
    ("contribution", "float64"),
];

/// Column schema of `ReturnContributionResult.to_factor_dataframe`.
const FACTOR_COLUMNS: &[ColumnSchema<'static>] = &[
    ("factor", "str"),
    ("exposure", "float64"),
    ("factor_return", "float64"),
    ("contribution", "float64"),
];

/// Read an optional float cell from a DataFrame record, treating ``None``
/// and ``NaN`` as absent.
fn optional_float(record: &Bound<'_, PyDict>, key: &str) -> PyResult<Option<f64>> {
    let Some(value) = record.get_item(key)? else {
        return Ok(None);
    };
    if value.is_none() {
        return Ok(None);
    }
    let value: f64 = value.extract().map_err(|_| {
        value_error(format!(
            "return contribution column {key:?} must hold floats or NaN"
        ))
    })?;
    Ok(if value.is_nan() { None } else { Some(value) })
}

/// Build one position row from a `DataFrame.to_dict("records")` entry.
fn position_from_record(record: &Bound<'_, PyDict>) -> PyResult<ReturnContributionPosition> {
    let id = record
        .get_item("id")?
        .ok_or_else(|| value_error("return contribution rows need an 'id' column or index"))?;
    let id: String = id.str()?.extract()?;
    let period_return = optional_float(record, "return")?
        .ok_or_else(|| value_error(format!("position {id:?} has no 'return' value")))?;
    let mut groups = BTreeMap::new();
    for (key, value) in record.iter() {
        let key: String = key.extract()?;
        if let Some(dimension) = key.strip_prefix("group:") {
            if value.is_none() {
                continue;
            }
            if let Ok(f) = value.extract::<f64>() {
                if f.is_nan() {
                    continue;
                }
            }
            groups.insert(dimension.to_owned(), value.str()?.extract()?);
        }
    }
    Ok(ReturnContributionPosition {
        id,
        market_value: optional_float(record, "market_value")?,
        weight: optional_float(record, "weight")?,
        period_return,
        groups,
        benchmark_weight: optional_float(record, "benchmark_weight")?,
        benchmark_return: optional_float(record, "benchmark_return")?,
    })
}

/// Read typed position rows from a pandas ``DataFrame``.
fn positions_from_dataframe(
    py: Python<'_>,
    frame: &Bound<'_, PyAny>,
) -> PyResult<Vec<ReturnContributionPosition>> {
    let columns: Vec<String> = frame
        .getattr("columns")?
        .call_method0("tolist")?
        .extract()?;
    let frame = if columns.iter().any(|c| c == "id") {
        frame.clone()
    } else {
        let kwargs = PyDict::new(py);
        kwargs.set_item("names", "id")?;
        frame
            .call_method("rename_axis", (), Some(&kwargs))?
            .call_method0("reset_index")?
    };
    let kwargs = PyDict::new(py);
    kwargs.set_item("orient", "records")?;
    let records: Vec<Bound<'_, PyDict>> =
        frame.call_method("to_dict", (), Some(&kwargs))?.extract()?;
    records.iter().map(position_from_record).collect()
}

/// Extract a [`ReturnContributionSpec`] from a dict, JSON string, or
/// pandas ``DataFrame``.
///
/// DataFrame rows stay typed so non-finite values reach Rust validation.
/// Dict/JSON inputs merge keywords before deserialization; duplicate fields
/// raise ``ValueError`` rather than silently overriding either value.
pub(crate) fn extract_return_contribution_spec(
    py: Python<'_>,
    spec: &Bound<'_, PyAny>,
    as_of: Option<&Bound<'_, PyAny>>,
    weighting: Option<&str>,
    factors: Option<&Bound<'_, PyAny>>,
) -> PyResult<ReturnContributionSpec> {
    let pd = py.import("pandas")?;
    let value = if let Ok(json) = spec.extract::<String>() {
        serde_json::from_str::<serde_json::Value>(&json)
            .map_err(|e| serde_json_to_py(e, "invalid return contribution JSON"))?
    } else if let Ok(dict) = spec.cast::<PyDict>() {
        let dict = dict.copy()?;
        if let Some(value) = dict.get_item("as_of")? {
            if !value.is_instance_of::<pyo3::types::PyString>() {
                dict.set_item("as_of", extract_date_iso(&value)?)?;
            }
        }
        py_to_json_value(py, dict.as_any(), "spec")?
    } else if spec.is_instance(&pd.getattr("DataFrame")?)? {
        let as_of = as_of
            .ok_or_else(|| value_error("as_of is required when spec is a pandas DataFrame"))?;
        return Ok(ReturnContributionSpec {
            as_of: extract_date_iso(as_of)?,
            positions: positions_from_dataframe(py, spec)?,
            weighting: weighting
                .map(|label| serde_json::from_value(serde_json::Value::String(label.to_owned())))
                .transpose()
                .map_err(|e| serde_json_to_py(e, "invalid return contribution spec"))?
                .unwrap_or_default(),
            factors: factors
                .map(|value| {
                    serde_json::from_value(py_to_json_value(py, value, "factors")?)
                        .map_err(|e| serde_json_to_py(e, "invalid return contribution spec"))
                })
                .transpose()?
                .unwrap_or_default(),
        });
    } else {
        return Err(PyTypeError::new_err(
            "spec must be a dict, a JSON str, or a pandas DataFrame of positions",
        ));
    };
    let serde_json::Value::Object(mut fields) = value else {
        return Err(value_error(
            "invalid return contribution spec: expected a JSON object",
        ));
    };
    let keywords = [
        (
            "as_of",
            as_of
                .map(|value| extract_date_iso(value).map(serde_json::Value::String))
                .transpose()?,
        ),
        (
            "weighting",
            weighting.map(|label| serde_json::Value::String(label.to_owned())),
        ),
        (
            "factors",
            factors
                .map(|value| py_to_json_value(py, value, "factors"))
                .transpose()?,
        ),
    ];
    for (key, keyword) in keywords {
        if let Some(keyword) = keyword {
            if fields.contains_key(key) {
                return Err(value_error(format!(
                    "{key} is given both in the spec and as a keyword argument; pass it once"
                )));
            }
            fields.insert(key.to_owned(), keyword);
        }
    }
    serde_json::from_value(serde_json::Value::Object(fields))
        .map_err(|e| serde_json_to_py(e, "invalid return contribution spec"))
}

/// Return-contribution attribution result.
///
/// Returned by ``attribute_return_contribution``. Decomposes a portfolio
/// return into per-instrument, per-group, and per-factor contributions, with
/// an optional Brinson-Fachler benchmark-relative block.
///
/// Examples
/// --------
/// >>> from finstack_quant.attribution import attribute_return_contribution
/// >>> res = attribute_return_contribution(
/// ...     {"as_of": "2026-01-02",
/// ...      "positions": [{"id": "A", "weight": 0.6, "return": 0.02},
/// ...                    {"id": "B", "weight": 0.4, "return": -0.01}]}
/// ... )
/// >>> round(res.portfolio_return, 6)
/// 0.008
/// >>> list(res.to_dataframe().columns)
/// ['id', 'weight', 'return', 'contribution', 'active_contribution']
#[pyclass(
    name = "ReturnContributionResult",
    module = "finstack_quant.attribution",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub(crate) struct PyReturnContributionResult {
    pub(crate) inner: finstack_quant_attribution::ReturnContributionResult,
}

#[pymethods]
impl PyReturnContributionResult {
    /// Total portfolio return, equal to the summed instrument contributions.
    #[getter]
    fn portfolio_return(&self) -> f64 {
        self.inner.portfolio_return
    }

    /// Per-instrument contribution rows.
    #[getter]
    fn instrument_contribution<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.instrument_contribution)
    }

    /// Contributions keyed by group dimension.
    #[getter]
    fn group_contribution<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.group_contribution)
    }

    /// Factor contribution rows.
    #[getter]
    fn factor_contribution<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.factor_contribution)
    }

    /// Idiosyncratic residual when factor rows were supplied:
    /// ``portfolio_return - sum(factor contributions)``. ``None`` when the
    /// spec carried no factors.
    #[getter]
    fn specific_return(&self) -> Option<f64> {
        self.inner.specific_return
    }

    /// Brinson-Fachler benchmark-relative block, when benchmark inputs were
    /// supplied.
    #[getter]
    fn benchmark_relative<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .benchmark_relative
            .as_ref()
            .map(|value| serde_to_py(py, value))
            .transpose()
    }

    /// Diagnostic warnings (for example leveraged weights from a near-flat
    /// net-market-value book).
    #[getter]
    fn warnings(&self) -> Vec<String> {
        self.inner.warnings.clone()
    }

    /// Per-instrument contributions as a ``pandas.DataFrame``.
    ///
    /// Columns: ``id``, ``weight``, ``return``, ``contribution``,
    /// ``active_contribution`` (``NaN`` when no benchmark was supplied).
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_rows_to_dataframe_with_schema(
            py,
            &self.inner.instrument_contribution,
            INSTRUMENT_COLUMNS,
        )
    }

    /// Group-bucket contributions as a long ``pandas.DataFrame``.
    ///
    /// Columns: ``dimension`` (the ``group:<dimension>`` label name),
    /// ``key`` (bucket), ``contribution``. Empty with schema columns when the
    /// spec carried no group labels.
    fn to_group_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let rows: Vec<serde_json::Value> = self
            .inner
            .group_contribution
            .iter()
            .flat_map(|(dimension, buckets)| {
                buckets.iter().map(move |bucket| {
                    serde_json::json!({
                        "dimension": dimension,
                        "key": bucket.key,
                        "contribution": bucket.contribution,
                    })
                })
            })
            .collect();
        serde_rows_to_dataframe_with_schema(py, &rows, GROUP_COLUMNS)
    }

    /// Factor contributions as a ``pandas.DataFrame``.
    ///
    /// Columns: ``factor``, ``exposure``, ``factor_return``,
    /// ``contribution``. Empty with schema columns when no factor rows were
    /// supplied.
    fn to_factor_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_rows_to_dataframe_with_schema(py, &self.inner.factor_contribution, FACTOR_COLUMNS)
    }

    /// Per-instrument contributions as a ``pandas.Series`` named
    /// ``contribution`` and indexed by instrument id.
    fn to_series<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let labels: Vec<String> = self
            .inner
            .instrument_contribution
            .iter()
            .map(|row| row.id.clone())
            .collect();
        let values: Vec<f64> = self
            .inner
            .instrument_contribution
            .iter()
            .map(|row| row.contribution)
            .collect();
        labeled_values_to_series(py, &labels, values, "contribution")
    }

    /// Serialize to a compact JSON string.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Deserialize from a JSON string produced by ``to_json``.
    ///
    /// Raises ``ValueError`` when the JSON does not match the result schema.
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: finstack_quant_attribution::ReturnContributionResult =
            serde_json::from_str(json)
                .map_err(|e| serde_json_to_py(e, "invalid ReturnContributionResult JSON"))?;
        Ok(Self { inner })
    }

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Identify this value in notebooks and logs.
    ///
    /// Rendered from the wire representation, so the fields shown are the
    /// fields `to_json()` names. Collections are summarised by length; use
    /// `to_json()` or a DataFrame exit when the contents matter.
    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("ReturnContributionResult", &self.inner)
    }

    /// Render as an HTML table in Jupyter notebooks (delegates to
    /// ``to_dataframe``; ``None`` falls back to ``__repr__``).
    fn _repr_html_(&self, py: Python<'_>) -> Option<String> {
        let frame = self.to_dataframe(py).ok()?;
        frame.call_method0("_repr_html_").ok()?.extract().ok()
    }
}
