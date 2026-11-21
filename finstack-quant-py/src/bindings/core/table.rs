//! Bindings for `finstack_quant_core::table`: Arrow interchange surface.
//!
//! Exposes `ArrowTable` and `LabeledSeries`, both backed by Arrow `RecordBatch`
//! and implementing the Arrow PyCapsule C-stream (`__arrow_c_stream__`) and
//! C-schema (`__arrow_c_schema__`) protocols so `pyarrow`, `polars`, `duckdb`,
//! and `pandas` can consume finstack results directly.
//!
//! Spec: <https://arrow.apache.org/docs/format/CDataInterface/PyCapsuleInterface.html>

use std::io::Cursor;

use arrow::array::{Array, Float64Array, Int64Array, StringArray};
use arrow::datatypes::DataType;
use arrow::ffi::FFI_ArrowSchema;
use arrow::ffi_stream::FFI_ArrowArrayStream;
use arrow::ipc::reader::StreamReader;
use arrow::ipc::writer::StreamWriter;
use arrow::record_batch::{RecordBatch, RecordBatchIterator};
use finstack_quant_core::table::TableEnvelope;
use numpy::PyArray1;
use pyo3::exceptions::{PyImportError, PyKeyError, PyTypeError};
use pyo3::prelude::*;
use pyo3::types::{PyBool, PyBytes, PyCapsule, PyDict, PyList, PyString};

use crate::bindings::pydoc::pydoc;
use crate::bindings::table_builders::{
    TableBuilder, record_batch_to_html, record_batch_to_pandas, record_batch_to_py_dict,
    record_batch_to_py_records,
};
use crate::errors::value_error;

#[doc = pydoc!("finstack_quant.core.table", "ArrowTable")]
#[pyclass(module = "finstack_quant.core.table", name = "ArrowTable", frozen)]
pub struct PyArrowTable {
    pub(crate) batch: RecordBatch,
}

impl PyArrowTable {
    /// Wrap a prebuilt Arrow `RecordBatch` in [`PyArrowTable`].
    pub(crate) fn from_record_batch(batch: RecordBatch) -> Self {
        Self { batch }
    }

    /// Convert a core table envelope into an Arrow-backed Python table.
    pub(crate) fn from_envelope(table: &TableEnvelope) -> PyResult<Self> {
        let batch =
            finstack_quant_arrow::to_record_batch(table).map_err(crate::errors::core_to_py)?;
        Ok(Self { batch })
    }

    pub(crate) fn arrow_c_stream<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyCapsule>> {
        let schema = self.batch.schema();
        let reader = RecordBatchIterator::new([Ok(self.batch.clone())], schema);
        let stream = FFI_ArrowArrayStream::new(Box::new(reader));
        PyCapsule::new_with_value(py, stream, c"arrow_array_stream")
    }

    pub(crate) fn arrow_c_schema<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyCapsule>> {
        let schema = FFI_ArrowSchema::try_from(self.batch.schema().as_ref())
            .map_err(|e| value_error(format!("Arrow C schema export failed: {e}")))?;
        PyCapsule::new_with_value(py, schema, c"arrow_schema")
    }

    pub(crate) fn render_html(&self) -> String {
        record_batch_to_html(&self.batch)
    }

    fn to_ipc_bytes(&self) -> PyResult<Vec<u8>> {
        let mut buffer = Vec::new();
        {
            let mut writer = StreamWriter::try_new(&mut buffer, &self.batch.schema())
                .map_err(|e| value_error(format!("Arrow IPC encode failed: {e}")))?;
            writer
                .write(&self.batch)
                .map_err(|e| value_error(format!("Arrow IPC encode failed: {e}")))?;
            writer
                .finish()
                .map_err(|e| value_error(format!("Arrow IPC encode failed: {e}")))?;
        }
        Ok(buffer)
    }

    fn decode_ipc(data: &[u8]) -> PyResult<Self> {
        let reader = StreamReader::try_new(Cursor::new(data), None)
            .map_err(|e| value_error(format!("Arrow IPC decode failed: {e}")))?;
        let schema = reader.schema();
        let batches: Vec<RecordBatch> = reader
            .collect::<Result<_, _>>()
            .map_err(|e| value_error(format!("Arrow IPC decode failed: {e}")))?;
        let batch = arrow::compute::concat_batches(&schema, &batches)
            .map_err(|e| value_error(format!("Arrow IPC decode failed: {e}")))?;
        Ok(Self { batch })
    }
}

#[pymethods]
impl PyArrowTable {
    #[doc = pydoc!("finstack_quant.core.table", "ArrowTable.num_rows")]
    #[getter]
    fn num_rows(&self) -> usize {
        self.batch.num_rows()
    }

    #[doc = pydoc!("finstack_quant.core.table", "ArrowTable.num_columns")]
    #[getter]
    fn num_columns(&self) -> usize {
        self.batch.num_columns()
    }

    #[doc = pydoc!("finstack_quant.core.table", "ArrowTable.shape")]
    #[getter]
    fn shape(&self) -> (usize, usize) {
        (self.batch.num_rows(), self.batch.num_columns())
    }

    fn __len__(&self) -> usize {
        self.batch.num_rows()
    }

    #[doc = pydoc!("finstack_quant.core.table", "ArrowTable.column_names")]
    #[pyo3(text_signature = "($self)")]
    fn column_names(&self) -> Vec<String> {
        self.batch
            .schema()
            .fields()
            .iter()
            .map(|f| f.name().clone())
            .collect()
    }

    #[doc = pydoc!("finstack_quant.core.table", "ArrowTable.schema")]
    #[pyo3(text_signature = "($self)")]
    fn schema(&self) -> Vec<(String, String, bool)> {
        self.batch
            .schema()
            .fields()
            .iter()
            .map(|f| (f.name().clone(), f.data_type().to_string(), f.is_nullable()))
            .collect()
    }

    fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
        other
            .extract::<PyRef<'_, PyArrowTable>>()
            .map(|rhs| self.batch == rhs.batch)
            .unwrap_or(false)
    }

    #[doc = pydoc!("finstack_quant.core.table", "ArrowTable.__arrow_c_stream__")]
    #[pyo3(signature = (requested_schema=None))]
    fn __arrow_c_stream__<'py>(
        &self,
        py: Python<'py>,
        requested_schema: Option<Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyCapsule>> {
        let _ = requested_schema; // schema negotiation unsupported; export native schema
        self.arrow_c_stream(py)
    }

    #[doc = pydoc!("finstack_quant.core.table", "ArrowTable.__arrow_c_schema__")]
    fn __arrow_c_schema__<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyCapsule>> {
        self.arrow_c_schema(py)
    }

    #[doc = pydoc!("finstack_quant.core.table", "ArrowTable.to_ipc")]
    #[pyo3(text_signature = "($self)")]
    fn to_ipc<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        Ok(PyBytes::new(py, &self.to_ipc_bytes()?))
    }

    #[doc = pydoc!("finstack_quant.core.table", "ArrowTable.from_ipc")]
    #[staticmethod]
    #[pyo3(text_signature = "(data)")]
    fn from_ipc(data: &[u8]) -> PyResult<Self> {
        Self::decode_ipc(data)
    }

    fn __reduce__<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<(Bound<'py, PyAny>, (Bound<'py, PyBytes>,))> {
        let from_ipc = py.get_type::<Self>().getattr("from_ipc")?;
        Ok((from_ipc, (self.to_ipc(py)?,)))
    }

    #[doc = pydoc!("finstack_quant.core.table", "ArrowTable.to_pyarrow")]
    #[pyo3(text_signature = "($self)")]
    pub(crate) fn to_pyarrow<'py>(
        slf: &Bound<'py, Self>,
        py: Python<'py>,
    ) -> PyResult<Bound<'py, PyAny>> {
        py.import("pyarrow")?.call_method1("table", (slf,))
    }

    #[doc = pydoc!("finstack_quant.core.table", "ArrowTable.to_polars")]
    #[pyo3(text_signature = "($self)")]
    pub(crate) fn to_polars<'py>(
        slf: &Bound<'py, Self>,
        py: Python<'py>,
    ) -> PyResult<Bound<'py, PyAny>> {
        py.import("polars")?.call_method1("DataFrame", (slf,))
    }

    #[doc = pydoc!("finstack_quant.core.table", "ArrowTable.to_pandas")]
    #[pyo3(text_signature = "($self)")]
    pub(crate) fn to_pandas<'py>(
        slf: &Bound<'py, Self>,
        py: Python<'py>,
    ) -> PyResult<Bound<'py, PyAny>> {
        match Self::to_pyarrow(slf, py) {
            Ok(pa_table) => pa_table.call_method0("to_pandas"),
            Err(err) if err.is_instance_of::<PyImportError>(py) => {
                record_batch_to_pandas(py, &slf.borrow().batch)
            }
            Err(err) => Err(err),
        }
    }

    #[doc = pydoc!("finstack_quant.core.table", "ArrowTable.to_dict")]
    #[pyo3(text_signature = "($self)")]
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        record_batch_to_py_dict(py, &self.batch)
    }

    #[doc = pydoc!("finstack_quant.core.table", "ArrowTable.to_records")]
    #[pyo3(text_signature = "($self)")]
    fn to_records<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyList>> {
        record_batch_to_py_records(py, &self.batch)
    }

    fn _repr_html_(&self) -> String {
        self.render_html()
    }

    fn __repr__(&self) -> String {
        format!(
            "ArrowTable(rows={}, columns={})",
            self.batch.num_rows(),
            self.batch.num_columns()
        )
    }
}

/// Internal value storage backing [`PyLabeledSeries`].
#[derive(Clone, Debug)]
enum SeriesValues {
    Float64(Vec<f64>),
    NullableFloat64(Vec<Option<f64>>),
    NullableInt64(Vec<Option<i64>>),
}

enum RawSeriesItem {
    Int(i64),
    Float(f64),
    Null,
}

fn floats_eq(a: f64, b: f64) -> bool {
    a == b || (a.is_nan() && b.is_nan())
}

fn opt_floats_eq(a: Option<f64>, b: Option<f64>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(x), Some(y)) => floats_eq(x, y),
        _ => false,
    }
}

fn series_values_eq(lhs: &SeriesValues, rhs: &SeriesValues) -> bool {
    match (lhs, rhs) {
        (SeriesValues::Float64(a), SeriesValues::Float64(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(&x, &y)| floats_eq(x, y))
        }
        (SeriesValues::NullableFloat64(a), SeriesValues::NullableFloat64(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(&x, &y)| opt_floats_eq(x, y))
        }
        (SeriesValues::NullableInt64(a), SeriesValues::NullableInt64(b)) => a == b,
        _ => false,
    }
}

fn validate_label_len(labels: &[String], values_len: usize) -> PyResult<()> {
    if labels.len() != values_len {
        return Err(value_error(format!(
            "index and values must have equal length (got {} and {values_len})",
            labels.len()
        )));
    }
    Ok(())
}

fn parse_raw_series_items(values: &Bound<'_, PyAny>) -> PyResult<Vec<RawSeriesItem>> {
    if values.is_instance_of::<PyString>() || values.is_instance_of::<PyBytes>() {
        return Err(PyTypeError::new_err(
            "values must be a sequence or array of numbers, not str or bytes",
        ));
    }
    let iter = values
        .try_iter()
        .map_err(|_| PyTypeError::new_err("values must be an iterable of int, float, or None"))?;
    let mut items = Vec::new();
    for entry in iter {
        let item = entry?;
        if item.is_none() {
            items.push(RawSeriesItem::Null);
            continue;
        }
        if item.is_instance_of::<PyBool>()
            || item
                .get_type()
                .name()
                .is_ok_and(|name| name == "bool" || name == "bool_")
        {
            return Err(PyTypeError::new_err(
                "LabeledSeries values cannot be booleans; pass numeric values or None",
            ));
        }
        if let Ok(i) = item.extract::<i64>() {
            items.push(RawSeriesItem::Int(i));
        } else if let Ok(f) = item.extract::<f64>() {
            items.push(RawSeriesItem::Float(f));
        } else {
            return Err(PyTypeError::new_err(
                "LabeledSeries values must be int, float, or None",
            ));
        }
    }
    Ok(items)
}

#[doc = pydoc!("finstack_quant.core.table", "LabeledSeries")]
#[pyclass(module = "finstack_quant.core.table", name = "LabeledSeries", frozen)]
pub struct PyLabeledSeries {
    name: String,
    index_name: String,
    labels: Vec<String>,
    values: SeriesValues,
}

impl PyLabeledSeries {
    /// Construct a non-nullable float64 [`PyLabeledSeries`] from Rust vectors.
    pub(crate) fn from_f64(
        name: impl Into<String>,
        labels: Vec<String>,
        values: Vec<f64>,
    ) -> PyResult<Self> {
        validate_label_len(&labels, values.len())?;
        Ok(Self {
            name: name.into(),
            index_name: "label".to_string(),
            labels,
            values: SeriesValues::Float64(values),
        })
    }

    /// Construct a nullable float64 [`PyLabeledSeries`] from Rust vectors.
    pub(crate) fn from_opt_f64(
        name: impl Into<String>,
        labels: Vec<String>,
        values: Vec<Option<f64>>,
    ) -> PyResult<Self> {
        validate_label_len(&labels, values.len())?;
        Ok(Self {
            name: name.into(),
            index_name: "label".to_string(),
            labels,
            values: SeriesValues::NullableFloat64(values),
        })
    }

    /// Construct a nullable int64 [`PyLabeledSeries`] from Rust vectors.
    pub(crate) fn from_opt_i64(
        name: impl Into<String>,
        labels: Vec<String>,
        values: Vec<Option<i64>>,
    ) -> PyResult<Self> {
        validate_label_len(&labels, values.len())?;
        Ok(Self {
            name: name.into(),
            index_name: "label".to_string(),
            labels,
            values: SeriesValues::NullableInt64(values),
        })
    }

    /// Set the Arrow index column name used by two-column table exports.
    pub(crate) fn with_index_name(mut self, index_name: impl Into<String>) -> Self {
        self.index_name = index_name.into();
        self
    }

    /// Materialize this labeled series as a two-column [`PyArrowTable`].
    pub(crate) fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        let builder = TableBuilder::new().utf8(&self.index_name, self.labels.clone())?;
        match &self.values {
            SeriesValues::Float64(vals) => builder.f64(&self.name, vals.clone())?.build(),
            SeriesValues::NullableFloat64(vals) => {
                builder.nullable_f64(&self.name, vals.clone())?.build()
            }
            SeriesValues::NullableInt64(vals) => {
                builder.nullable_i64(&self.name, vals.clone())?.build()
            }
        }
    }

    fn py_value_at<'py>(&self, py: Python<'py>, idx: usize) -> PyResult<Bound<'py, PyAny>> {
        match &self.values {
            SeriesValues::Float64(vals) => Ok(vals[idx].into_pyobject(py)?.into_any()),
            SeriesValues::NullableFloat64(vals) => Ok(vals[idx].into_pyobject(py)?.into_any()),
            SeriesValues::NullableInt64(vals) => Ok(vals[idx].into_pyobject(py)?.into_any()),
        }
    }

    fn values_py_list<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyList>> {
        match &self.values {
            SeriesValues::Float64(vals) => PyList::new(py, vals),
            SeriesValues::NullableFloat64(vals) => PyList::new(py, vals),
            SeriesValues::NullableInt64(vals) => PyList::new(py, vals),
        }
    }
}

crate::bindings::macros::tabular_methods!(
    PyLabeledSeries,
    "LabeledSeries",
    module = "finstack_quant.core.table",
    custom_pandas
);

#[pymethods]
impl PyLabeledSeries {
    #[new]
    #[pyo3(signature = (index, values, *, name = "value", index_name = "label", dtype = None))]
    fn new(
        index: Vec<String>,
        values: &Bound<'_, PyAny>,
        name: &str,
        index_name: &str,
        dtype: Option<&str>,
    ) -> PyResult<Self> {
        let items = parse_raw_series_items(values)?;
        validate_label_len(&index, items.len())?;

        let has_float = items
            .iter()
            .any(|item| matches!(item, RawSeriesItem::Float(_)));
        let has_int = items
            .iter()
            .any(|item| matches!(item, RawSeriesItem::Int(_)));
        let has_null = items.iter().any(|item| matches!(item, RawSeriesItem::Null));

        let use_int64 = match dtype {
            Some("int64") => {
                if has_float {
                    return Err(PyTypeError::new_err(
                        "dtype='int64' requires integer values or None",
                    ));
                }
                true
            }
            Some("float64") => false,
            Some(other) => {
                return Err(value_error(format!(
                    "unsupported dtype '{other}'; expected 'float64' or 'int64'"
                )));
            }
            None => has_int && !has_float,
        };

        let series = if use_int64 {
            let ints = items
                .into_iter()
                .map(|item| match item {
                    RawSeriesItem::Int(i) => Ok(Some(i)),
                    RawSeriesItem::Null => Ok(None),
                    RawSeriesItem::Float(_) => Err(PyTypeError::new_err(
                        "dtype='int64' requires integer values or None",
                    )),
                })
                .collect::<PyResult<Vec<_>>>()?;
            Self::from_opt_i64(name, index, ints)?
        } else if has_null {
            let floats = items
                .into_iter()
                .map(|item| match item {
                    RawSeriesItem::Int(i) => Some(i as f64),
                    RawSeriesItem::Float(f) => Some(f),
                    RawSeriesItem::Null => None,
                })
                .collect();
            Self::from_opt_f64(name, index, floats)?
        } else {
            let floats = items
                .into_iter()
                .map(|item| match item {
                    RawSeriesItem::Int(i) => i as f64,
                    RawSeriesItem::Float(f) => f,
                    RawSeriesItem::Null => f64::NAN,
                })
                .collect();
            Self::from_f64(name, index, floats)?
        };
        Ok(series.with_index_name(index_name))
    }

    #[doc = pydoc!("finstack_quant.core.table", "LabeledSeries.name")]
    #[getter]
    fn name(&self) -> &str {
        &self.name
    }

    #[doc = pydoc!("finstack_quant.core.table", "LabeledSeries.index_name")]
    #[getter]
    fn index_name(&self) -> &str {
        &self.index_name
    }

    #[doc = pydoc!("finstack_quant.core.table", "LabeledSeries.index")]
    #[getter]
    fn index(&self) -> Vec<String> {
        self.labels.clone()
    }

    fn __len__(&self) -> usize {
        self.labels.len()
    }

    fn __contains__(&self, label: &Bound<'_, PyAny>) -> bool {
        label
            .extract::<&str>()
            .is_ok_and(|key| self.labels.iter().any(|item| item == key))
    }

    fn __getitem__<'py>(
        &self,
        py: Python<'py>,
        label: &Bound<'_, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        if let Ok(key) = label.extract::<&str>()
            && let Some(idx) = self.labels.iter().position(|item| item == key)
        {
            return self.py_value_at(py, idx);
        }
        Err(PyKeyError::new_err(label.clone().unbind()))
    }

    #[doc = pydoc!("finstack_quant.core.table", "LabeledSeries.get")]
    #[pyo3(signature = (label, default = None), text_signature = "($self, label, default=None)")]
    fn get<'py>(
        &self,
        py: Python<'py>,
        label: &Bound<'_, PyAny>,
        default: Option<Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        if let Ok(key) = label.extract::<&str>()
            && let Some(idx) = self.labels.iter().position(|item| item == key)
        {
            return self.py_value_at(py, idx);
        }
        Ok(default.unwrap_or_else(|| py.None().into_bound(py)))
    }

    #[doc = pydoc!("finstack_quant.core.table", "LabeledSeries.keys")]
    #[pyo3(text_signature = "($self)")]
    fn keys(&self) -> Vec<String> {
        self.labels.clone()
    }

    #[doc = pydoc!("finstack_quant.core.table", "LabeledSeries.values")]
    #[pyo3(text_signature = "($self)")]
    fn values<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyList>> {
        self.values_py_list(py)
    }

    #[doc = pydoc!("finstack_quant.core.table", "LabeledSeries.items")]
    #[pyo3(text_signature = "($self)")]
    fn items<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyList>> {
        let mut pairs = Vec::with_capacity(self.labels.len());
        for (idx, label) in self.labels.iter().enumerate() {
            pairs.push((label.as_str(), self.py_value_at(py, idx)?));
        }
        PyList::new(py, pairs)
    }

    fn __iter__<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        PyList::new(py, &self.labels)?.call_method0("__iter__")
    }

    #[doc = pydoc!("finstack_quant.core.table", "LabeledSeries.to_dict")]
    #[pyo3(text_signature = "($self)")]
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let dict = PyDict::new(py);
        for (idx, label) in self.labels.iter().enumerate() {
            dict.set_item(label.as_str(), self.py_value_at(py, idx)?)?;
        }
        Ok(dict)
    }

    #[doc = pydoc!("finstack_quant.core.table", "LabeledSeries.to_list")]
    #[pyo3(text_signature = "($self)")]
    fn to_list<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyList>> {
        self.values_py_list(py)
    }

    #[doc = pydoc!("finstack_quant.core.table", "LabeledSeries.to_numpy")]
    #[pyo3(text_signature = "($self)")]
    fn to_numpy<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        match &self.values {
            SeriesValues::Float64(vals) => Ok(PyArray1::from_slice(py, vals).into_any()),
            SeriesValues::NullableFloat64(vals) => {
                let floats: Vec<f64> = vals.iter().map(|v| v.unwrap_or(f64::NAN)).collect();
                Ok(PyArray1::from_vec(py, floats).into_any())
            }
            SeriesValues::NullableInt64(vals) => {
                if vals.iter().all(Option::is_some) {
                    let ints: Vec<i64> = vals.iter().filter_map(|v| *v).collect();
                    Ok(PyArray1::from_vec(py, ints).into_any())
                } else {
                    let py_vals = PyList::new(py, vals)?;
                    let np = py.import("numpy")?;
                    let kwargs = PyDict::new(py);
                    kwargs.set_item("dtype", "object")?;
                    np.call_method("array", (py_vals,), Some(&kwargs))
                }
            }
        }
    }

    #[doc = pydoc!("finstack_quant.core.table", "LabeledSeries.to_pandas")]
    #[pyo3(text_signature = "($self)")]
    fn to_pandas<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let pd = py.import("pandas")?;
        let kwargs = PyDict::new(py);
        kwargs.set_item("index", &self.labels)?;
        kwargs.set_item("name", &self.name)?;
        match &self.values {
            SeriesValues::Float64(vals) => {
                kwargs.set_item("dtype", "float64")?;
                pd.call_method("Series", (PyArray1::from_slice(py, vals),), Some(&kwargs))
            }
            SeriesValues::NullableFloat64(vals) => {
                let floats: Vec<f64> = vals.iter().map(|v| v.unwrap_or(f64::NAN)).collect();
                kwargs.set_item("dtype", "float64")?;
                pd.call_method("Series", (PyArray1::from_vec(py, floats),), Some(&kwargs))
            }
            SeriesValues::NullableInt64(vals) => {
                kwargs.set_item("dtype", "Int64")?;
                pd.call_method("Series", (PyList::new(py, vals)?,), Some(&kwargs))
            }
        }
    }

    #[doc = pydoc!("finstack_quant.core.table", "LabeledSeries.to_ipc")]
    #[pyo3(text_signature = "($self)")]
    fn to_ipc<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        let bytes = self.to_arrow_table()?.to_ipc_bytes()?;
        Ok(PyBytes::new(py, &bytes))
    }

    #[doc = pydoc!("finstack_quant.core.table", "LabeledSeries.from_ipc")]
    #[staticmethod]
    #[pyo3(text_signature = "(data)")]
    fn from_ipc(data: &[u8]) -> PyResult<Self> {
        let table = PyArrowTable::decode_ipc(data)?;
        let batch = &table.batch;
        if batch.num_columns() != 2 {
            return Err(value_error(format!(
                "LabeledSeries IPC stream must contain 2 columns, got {}",
                batch.num_columns()
            )));
        }
        let schema = batch.schema();
        let index_field = schema.field(0);
        if *index_field.data_type() != DataType::Utf8 {
            return Err(value_error(format!(
                "LabeledSeries index column '{}' must have Utf8 dtype, got {}",
                index_field.name(),
                index_field.data_type()
            )));
        }
        let label_arr = batch
            .column(0)
            .as_any()
            .downcast_ref::<StringArray>()
            .ok_or_else(|| value_error("failed to downcast LabeledSeries index column"))?;
        if label_arr.null_count() > 0 {
            return Err(value_error(
                "LabeledSeries label column cannot contain nulls",
            ));
        }
        let labels: Vec<String> = (0..label_arr.len())
            .map(|i| label_arr.value(i).to_string())
            .collect();

        let value_field = schema.field(1);
        let name = value_field.name().clone();
        let series = match value_field.data_type() {
            DataType::Float64 => {
                let arr = batch
                    .column(1)
                    .as_any()
                    .downcast_ref::<Float64Array>()
                    .ok_or_else(|| {
                        value_error("failed to downcast LabeledSeries Float64 column")
                    })?;
                if !value_field.is_nullable() && arr.null_count() == 0 {
                    let vals = (0..arr.len()).map(|i| arr.value(i)).collect();
                    Self::from_f64(name, labels, vals)?
                } else {
                    let vals = (0..arr.len())
                        .map(|i| (!arr.is_null(i)).then(|| arr.value(i)))
                        .collect();
                    Self::from_opt_f64(name, labels, vals)?
                }
            }
            DataType::Int64 => {
                let arr = batch
                    .column(1)
                    .as_any()
                    .downcast_ref::<Int64Array>()
                    .ok_or_else(|| value_error("failed to downcast LabeledSeries Int64 column"))?;
                let vals = (0..arr.len())
                    .map(|i| (!arr.is_null(i)).then(|| arr.value(i)))
                    .collect();
                Self::from_opt_i64(name, labels, vals)?
            }
            other => {
                return Err(value_error(format!(
                    "unsupported LabeledSeries Arrow value dtype: {other}"
                )));
            }
        };
        Ok(series.with_index_name(index_field.name()))
    }

    fn __reduce__<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<(Bound<'py, PyAny>, (Bound<'py, PyBytes>,))> {
        let from_ipc = py.get_type::<Self>().getattr("from_ipc")?;
        Ok((from_ipc, (self.to_ipc(py)?,)))
    }

    fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
        other
            .extract::<PyRef<'_, PyLabeledSeries>>()
            .map(|rhs| {
                self.name == rhs.name
                    && self.index_name == rhs.index_name
                    && self.labels == rhs.labels
                    && series_values_eq(&self.values, &rhs.values)
            })
            .unwrap_or(false)
    }

    fn __repr__(&self) -> String {
        format!(
            "LabeledSeries(name={:?}, len={})",
            self.name,
            self.labels.len()
        )
    }
}

/// Build the `finstack_quant.core.table` submodule.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = crate::bindings::module_utils::new_submodule(parent, "table")?;
    m.setattr("__doc__", pydoc!("finstack_quant.core.table", "__module__"))?;
    m.add_class::<PyArrowTable>()?;
    m.add_class::<PyLabeledSeries>()?;
    let all = PyList::new(py, ["ArrowTable", "LabeledSeries"])?;
    m.setattr("__all__", all)?;
    crate::bindings::module_utils::attach_submodule(
        parent,
        &m,
        crate::bindings::module_utils::Exposure::Compiled,
    )?;
    Ok(())
}
