//! Shared Rust-to-Arrow table builders and Arrow-to-Python converters.
//!
//! Keeps Arrow `RecordBatch` construction and zero-dependency pandas / dict /
//! HTML rendering in one module so individual binding domains stay small and
//! free of per-crate DataFrame boilerplate.

use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;
use std::sync::Arc;

use arrow::array::{
    Array, ArrayRef, BooleanArray, Date32Array, Float64Array, Int64Array, LargeStringArray,
    StringArray, UInt32Array,
};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::record_batch::{RecordBatch, RecordBatchOptions};
use numpy::PyArray1;
use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyDict, PyList, PyString};

use crate::bindings::core::table::PyArrowTable;
use crate::bindings::date_utils::date_to_py;
use crate::errors::value_error;

/// Julian day number of `1970-01-01` (`time::Date::from_julian_day`).
const UNIX_EPOCH_JULIAN_DAY: i32 = 2_440_588;

static PANDAS_DATAFRAME: PyOnceLock<Py<PyAny>> = PyOnceLock::new();
static PANDAS_SERIES: PyOnceLock<Py<PyAny>> = PyOnceLock::new();

fn pandas_dataframe<'py>(py: Python<'py>) -> PyResult<&'py Bound<'py, PyAny>> {
    PANDAS_DATAFRAME
        .get_or_try_init(py, || {
            Ok(py.import("pandas")?.getattr("DataFrame")?.unbind())
        })
        .map(|ctor| ctor.bind(py))
}

fn pandas_series<'py>(py: Python<'py>) -> PyResult<&'py Bound<'py, PyAny>> {
    PANDAS_SERIES
        .get_or_try_init(py, || Ok(py.import("pandas")?.getattr("Series")?.unbind()))
        .map(|ctor| ctor.bind(py))
}

fn date32_to_time_date(days: i32) -> PyResult<time::Date> {
    let julian_day = UNIX_EPOCH_JULIAN_DAY
        .checked_add(days)
        .ok_or_else(|| value_error(format!("Date32 day offset out of range: {days}")))?;
    time::Date::from_julian_day(julian_day)
        .map_err(|e| value_error(format!("invalid Date32 day offset {days}: {e}")))
}

/// Builder constructing an Arrow [`PyArrowTable`] from typed Rust columns.
///
/// Validates consistent column lengths on every appended column and preserves
/// column declaration order and nullability in the resulting Arrow schema.
pub(crate) struct TableBuilder {
    fields: Vec<Field>,
    arrays: Vec<ArrayRef>,
    row_count: Option<usize>,
    metadata: HashMap<String, String>,
}

impl TableBuilder {
    /// Create an empty builder with no columns.
    pub(crate) fn new() -> Self {
        Self {
            fields: Vec::new(),
            arrays: Vec::new(),
            row_count: None,
            metadata: HashMap::new(),
        }
    }

    fn push_column(
        &mut self,
        name: &str,
        data_type: DataType,
        nullable: bool,
        array: ArrayRef,
    ) -> PyResult<()> {
        let len = array.len();
        match self.row_count {
            Some(expected) if expected != len => {
                return Err(value_error(format!(
                    "column '{name}' length {len} does not match expected row count {expected}"
                )));
            }
            None => self.row_count = Some(len),
            Some(_) => {}
        }
        self.fields.push(Field::new(name, data_type, nullable));
        self.arrays.push(array);
        Ok(())
    }

    /// Append a non-nullable UTF-8 string column.
    pub(crate) fn utf8(mut self, name: &str, values: Vec<String>) -> PyResult<Self> {
        let array: ArrayRef = Arc::new(StringArray::from(values));
        self.push_column(name, DataType::Utf8, false, array)?;
        Ok(self)
    }

    /// Append a nullable UTF-8 string column.
    pub(crate) fn nullable_utf8(
        mut self,
        name: &str,
        values: Vec<Option<String>>,
    ) -> PyResult<Self> {
        let array: ArrayRef = Arc::new(StringArray::from(values));
        self.push_column(name, DataType::Utf8, true, array)?;
        Ok(self)
    }

    /// Append a non-nullable `Float64` column.
    pub(crate) fn f64(mut self, name: &str, values: Vec<f64>) -> PyResult<Self> {
        let array: ArrayRef = Arc::new(Float64Array::from(values));
        self.push_column(name, DataType::Float64, false, array)?;
        Ok(self)
    }

    /// Append a nullable `Float64` column.
    pub(crate) fn nullable_f64(mut self, name: &str, values: Vec<Option<f64>>) -> PyResult<Self> {
        let array: ArrayRef = Arc::new(Float64Array::from(values));
        self.push_column(name, DataType::Float64, true, array)?;
        Ok(self)
    }

    /// Append a non-nullable `Int64` column.
    pub(crate) fn i64(mut self, name: &str, values: Vec<i64>) -> PyResult<Self> {
        let array: ArrayRef = Arc::new(Int64Array::from(values));
        self.push_column(name, DataType::Int64, false, array)?;
        Ok(self)
    }

    /// Append a nullable `Int64` column.
    pub(crate) fn nullable_i64(mut self, name: &str, values: Vec<Option<i64>>) -> PyResult<Self> {
        let array: ArrayRef = Arc::new(Int64Array::from(values));
        self.push_column(name, DataType::Int64, true, array)?;
        Ok(self)
    }

    /// Append a nullable `UInt32` column.
    pub(crate) fn nullable_u32(mut self, name: &str, values: Vec<Option<u32>>) -> PyResult<Self> {
        let array: ArrayRef = Arc::new(UInt32Array::from(values));
        self.push_column(name, DataType::UInt32, true, array)?;
        Ok(self)
    }

    /// Append a non-nullable `Boolean` column.
    pub(crate) fn bool(mut self, name: &str, values: Vec<bool>) -> PyResult<Self> {
        let array: ArrayRef = Arc::new(BooleanArray::from(values));
        self.push_column(name, DataType::Boolean, false, array)?;
        Ok(self)
    }

    /// Append a nullable `Boolean` column.
    pub(crate) fn nullable_bool(mut self, name: &str, values: Vec<Option<bool>>) -> PyResult<Self> {
        let array: ArrayRef = Arc::new(BooleanArray::from(values));
        self.push_column(name, DataType::Boolean, true, array)?;
        Ok(self)
    }

    /// Append a non-nullable `Date32` column from Rust dates.
    pub(crate) fn date(
        mut self,
        name: &str,
        dates: &[finstack_quant_core::dates::Date],
    ) -> PyResult<Self> {
        let days: Vec<i32> = dates
            .iter()
            .map(|d| d.to_julian_day() - UNIX_EPOCH_JULIAN_DAY)
            .collect();
        let array: ArrayRef = Arc::new(Date32Array::from(days));
        self.push_column(name, DataType::Date32, false, array)?;
        Ok(self)
    }

    /// Append a nullable `Date32` column from optional Rust dates.
    pub(crate) fn nullable_date(
        mut self,
        name: &str,
        dates: &[Option<finstack_quant_core::dates::Date>],
    ) -> PyResult<Self> {
        let days: Vec<Option<i32>> = dates
            .iter()
            .map(|opt| opt.map(|d| d.to_julian_day() - UNIX_EPOCH_JULIAN_DAY))
            .collect();
        let array: ArrayRef = Arc::new(Date32Array::from(days));
        self.push_column(name, DataType::Date32, true, array)?;
        Ok(self)
    }

    /// Finalize the columns into a [`PyArrowTable`].
    pub(crate) fn build(self) -> PyResult<PyArrowTable> {
        let schema = Arc::new(Schema::new_with_metadata(self.fields, self.metadata));
        let options = RecordBatchOptions::new().with_row_count(Some(self.row_count.unwrap_or(0)));
        let batch = RecordBatch::try_new_with_options(schema, self.arrays, &options)
            .map_err(|e| value_error(format!("Arrow record batch build failed: {e}")))?;
        Ok(PyArrowTable::from_record_batch(batch))
    }
}

#[derive(Clone, Copy)]
enum SerdeColumnKind {
    Utf8,
    Float64,
    Int64,
    UInt32,
    Boolean,
}

fn parse_serde_column_kind(dtype: &str) -> PyResult<SerdeColumnKind> {
    match dtype {
        "str" | "string" => Ok(SerdeColumnKind::Utf8),
        "float64" | "Float64" => Ok(SerdeColumnKind::Float64),
        "int64" | "Int64" => Ok(SerdeColumnKind::Int64),
        "uint32" | "UInt32" => Ok(SerdeColumnKind::UInt32),
        "bool" | "boolean" => Ok(SerdeColumnKind::Boolean),
        other => Err(value_error(format!(
            "unsupported column dtype '{other}' in serde_rows_to_arrow_table"
        ))),
    }
}

fn infer_column_kind(
    maps: &[serde_json::Map<String, serde_json::Value>],
    key: &str,
) -> SerdeColumnKind {
    let mut saw_bool = false;
    let mut saw_int = false;
    let mut saw_float = false;
    let mut saw_other = false;

    for map in maps {
        let Some(val) = map.get(key) else {
            continue;
        };
        match val {
            serde_json::Value::Null => {}
            serde_json::Value::Bool(_) => saw_bool = true,
            serde_json::Value::Number(num) => {
                if num.is_i64() || num.is_u64() {
                    saw_int = true;
                } else {
                    saw_float = true;
                }
            }
            _ => saw_other = true,
        }
    }

    if saw_other {
        SerdeColumnKind::Utf8
    } else if saw_bool && !saw_int && !saw_float {
        SerdeColumnKind::Boolean
    } else if saw_int && !saw_bool && !saw_float {
        SerdeColumnKind::Int64
    } else if !saw_bool && (saw_int || saw_float) {
        SerdeColumnKind::Float64
    } else if !saw_bool && !saw_int && !saw_float {
        // All-null columns stay Float64 to match pandas / numpy numeric expectations.
        SerdeColumnKind::Float64
    } else {
        SerdeColumnKind::Utf8
    }
}

fn push_serde_column(
    builder: TableBuilder,
    maps: &[serde_json::Map<String, serde_json::Value>],
    name: &str,
    kind: SerdeColumnKind,
) -> PyResult<TableBuilder> {
    match kind {
        SerdeColumnKind::Utf8 => {
            let mut vals = Vec::with_capacity(maps.len());
            for map in maps {
                let cell = match map.get(name) {
                    None | Some(serde_json::Value::Null) => None,
                    Some(serde_json::Value::String(s)) => Some(s.clone()),
                    Some(other) => Some(other.to_string()),
                };
                vals.push(cell);
            }
            builder.nullable_utf8(name, vals)
        }
        SerdeColumnKind::Float64 => {
            let mut vals = Vec::with_capacity(maps.len());
            for map in maps {
                let cell = match map.get(name) {
                    None | Some(serde_json::Value::Null) => None,
                    Some(v) => Some(v.as_f64().ok_or_else(|| {
                        value_error(format!("column '{name}' expected float or null, got {v}"))
                    })?),
                };
                vals.push(cell);
            }
            builder.nullable_f64(name, vals)
        }
        SerdeColumnKind::Int64 => {
            let mut vals = Vec::with_capacity(maps.len());
            for map in maps {
                let cell = match map.get(name) {
                    None | Some(serde_json::Value::Null) => None,
                    Some(v) => Some(
                        v.as_i64()
                            .or_else(|| v.as_u64().and_then(|u| i64::try_from(u).ok()))
                            .ok_or_else(|| {
                                value_error(format!(
                                    "column '{name}' expected int64 or null, got {v}"
                                ))
                            })?,
                    ),
                };
                vals.push(cell);
            }
            builder.nullable_i64(name, vals)
        }
        SerdeColumnKind::UInt32 => {
            let mut vals = Vec::with_capacity(maps.len());
            for map in maps {
                let cell = match map.get(name) {
                    None | Some(serde_json::Value::Null) => None,
                    Some(v) => Some(v.as_u64().and_then(|u| u32::try_from(u).ok()).ok_or_else(
                        || value_error(format!("column '{name}' expected uint32 or null, got {v}")),
                    )?),
                };
                vals.push(cell);
            }
            builder.nullable_u32(name, vals)
        }
        SerdeColumnKind::Boolean => {
            let mut vals = Vec::with_capacity(maps.len());
            for map in maps {
                let cell = match map.get(name) {
                    None | Some(serde_json::Value::Null) => None,
                    Some(v) => Some(v.as_bool().ok_or_else(|| {
                        value_error(format!("column '{name}' expected bool or null, got {v}"))
                    })?),
                };
                vals.push(cell);
            }
            builder.nullable_bool(name, vals)
        }
    }
}

/// Build an Arrow [`PyArrowTable`] from serializable row objects and an
/// authoritative column schema, preserving declared column order and empty dtypes.
///
/// Any extra keys emitted by rows that are not listed in `columns` are appended
/// after the declared columns in deterministic first-seen order with inferred
/// Arrow types.
pub(crate) fn serde_rows_to_arrow_table<T: serde::Serialize>(
    rows: &[T],
    columns: &[(&str, &str)],
) -> PyResult<PyArrowTable> {
    let mut maps = Vec::with_capacity(rows.len());
    for row in rows {
        let value = serde_json::to_value(row).map_err(crate::errors::display_to_py)?;
        let serde_json::Value::Object(map) = value else {
            return Err(value_error(
                "serde_rows_to_arrow_table expected each row to serialize as a JSON object",
            ));
        };
        maps.push(map);
    }

    let mut builder = TableBuilder::new();
    let mut seen: HashSet<String> = HashSet::with_capacity(columns.len());
    for &(name, dtype) in columns {
        let kind = parse_serde_column_kind(dtype)?;
        builder = push_serde_column(builder, &maps, name, kind)?;
        seen.insert(name.to_string());
    }

    let mut extra_keys: Vec<String> = Vec::new();
    for map in &maps {
        for key in map.keys() {
            if seen.insert(key.clone()) {
                extra_keys.push(key.clone());
            }
        }
    }
    for key in &extra_keys {
        let kind = infer_column_kind(&maps, key);
        builder = push_serde_column(builder, &maps, key, kind)?;
    }

    builder.build()
}

/// Build a single-row Arrow [`PyArrowTable`] from a serializable object and an
/// authoritative ordered column schema.
pub(crate) fn serde_object_to_single_row_arrow_table<T: serde::Serialize>(
    value: &T,
    columns: &[(&str, &str)],
) -> PyResult<PyArrowTable> {
    serde_rows_to_arrow_table(std::slice::from_ref(value), columns)
}

enum ColumnView<'a> {
    Utf8(&'a StringArray),
    LargeUtf8(&'a LargeStringArray),
    Float64(&'a Float64Array),
    Int64(&'a Int64Array),
    UInt32(&'a UInt32Array),
    Boolean(&'a BooleanArray),
    Date32(&'a Date32Array),
    Unsupported(&'a dyn Array),
}

fn downcast<'a, T: Array + 'static>(array: &'a ArrayRef, expected: &str) -> PyResult<&'a T> {
    array
        .as_any()
        .downcast_ref::<T>()
        .ok_or_else(|| value_error(format!("internal Arrow downcast mismatch for {expected}")))
}

impl<'a> ColumnView<'a> {
    fn from_array(array: &'a ArrayRef) -> PyResult<Self> {
        Ok(match array.data_type() {
            DataType::Utf8 => Self::Utf8(downcast::<StringArray>(array, "Utf8")?),
            DataType::LargeUtf8 => {
                Self::LargeUtf8(downcast::<LargeStringArray>(array, "LargeUtf8")?)
            }
            DataType::Float64 => Self::Float64(downcast::<Float64Array>(array, "Float64")?),
            DataType::Int64 => Self::Int64(downcast::<Int64Array>(array, "Int64")?),
            DataType::UInt32 => Self::UInt32(downcast::<UInt32Array>(array, "UInt32")?),
            DataType::Boolean => Self::Boolean(downcast::<BooleanArray>(array, "Boolean")?),
            DataType::Date32 => Self::Date32(downcast::<Date32Array>(array, "Date32")?),
            _ => Self::Unsupported(array.as_ref()),
        })
    }

    fn format_cell(&self, row: usize) -> String {
        let is_null = match self {
            Self::Utf8(arr) => arr.is_null(row),
            Self::LargeUtf8(arr) => arr.is_null(row),
            Self::Float64(arr) => arr.is_null(row),
            Self::Int64(arr) => arr.is_null(row),
            Self::UInt32(arr) => arr.is_null(row),
            Self::Boolean(arr) => arr.is_null(row),
            Self::Date32(arr) => arr.is_null(row),
            Self::Unsupported(arr) => arr.is_null(row),
        };
        if is_null {
            return "None".to_string();
        }
        match self {
            Self::Utf8(arr) => arr.value(row).to_string(),
            Self::LargeUtf8(arr) => arr.value(row).to_string(),
            Self::Float64(arr) => arr.value(row).to_string(),
            Self::Int64(arr) => arr.value(row).to_string(),
            Self::UInt32(arr) => arr.value(row).to_string(),
            Self::Boolean(arr) => {
                if arr.value(row) {
                    "True".to_string()
                } else {
                    "False".to_string()
                }
            }
            Self::Date32(arr) => {
                let days = arr.value(row);
                date32_to_time_date(days)
                    .map_or_else(|_| format!("Date32({days})"), |d| d.to_string())
            }
            Self::Unsupported(arr) => format!("<{}>", arr.data_type()),
        }
    }

    fn to_py_list<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyList>> {
        match self {
            Self::Utf8(arr) => {
                let vals: Vec<Option<&str>> = (0..arr.len())
                    .map(|i| (!arr.is_null(i)).then(|| arr.value(i)))
                    .collect();
                PyList::new(py, vals)
            }
            Self::LargeUtf8(arr) => {
                let vals: Vec<Option<&str>> = (0..arr.len())
                    .map(|i| (!arr.is_null(i)).then(|| arr.value(i)))
                    .collect();
                PyList::new(py, vals)
            }
            Self::Float64(arr) => {
                let vals: Vec<Option<f64>> = (0..arr.len())
                    .map(|i| (!arr.is_null(i)).then(|| arr.value(i)))
                    .collect();
                PyList::new(py, vals)
            }
            Self::Int64(arr) => {
                let vals: Vec<Option<i64>> = (0..arr.len())
                    .map(|i| (!arr.is_null(i)).then(|| arr.value(i)))
                    .collect();
                PyList::new(py, vals)
            }
            Self::UInt32(arr) => {
                let vals: Vec<Option<u32>> = (0..arr.len())
                    .map(|i| (!arr.is_null(i)).then(|| arr.value(i)))
                    .collect();
                PyList::new(py, vals)
            }
            Self::Boolean(arr) => {
                let vals: Vec<Option<bool>> = (0..arr.len())
                    .map(|i| (!arr.is_null(i)).then(|| arr.value(i)))
                    .collect();
                PyList::new(py, vals)
            }
            Self::Date32(arr) => {
                let vals = (0..arr.len())
                    .map(|i| {
                        if arr.is_null(i) {
                            Ok(None)
                        } else {
                            Ok(Some(date_to_py(py, date32_to_time_date(arr.value(i))?)?))
                        }
                    })
                    .collect::<PyResult<Vec<Option<Bound<'py, PyAny>>>>>()?;
                PyList::new(py, vals)
            }
            Self::Unsupported(arr) => {
                let vals: Vec<Option<String>> = (0..arr.len())
                    .map(|i| (!arr.is_null(i)).then(|| format!("<{}>", arr.data_type())))
                    .collect();
                PyList::new(py, vals)
            }
        }
    }

    fn to_pandas_series<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let series_ctor = pandas_series(py)?;
        let make_typed =
            |data: Bound<'py, PyAny>, dtype: Option<&str>| -> PyResult<Bound<'py, PyAny>> {
                let kwargs = PyDict::new(py);
                if let Some(dtype) = dtype {
                    kwargs.set_item("dtype", dtype)?;
                }
                series_ctor.call((data,), Some(&kwargs))
            };

        match self {
            Self::Utf8(_) | Self::LargeUtf8(_) => {
                make_typed(self.to_py_list(py)?.into_any(), Some("str"))
            }
            Self::Float64(arr) => {
                let vals: Vec<f64> = (0..arr.len())
                    .map(|i| {
                        if arr.is_null(i) {
                            f64::NAN
                        } else {
                            arr.value(i)
                        }
                    })
                    .collect();
                make_typed(PyArray1::from_vec(py, vals).into_any(), Some("float64"))
            }
            Self::Int64(arr) => {
                if arr.null_count() == 0 {
                    let vals: Vec<i64> = (0..arr.len()).map(|i| arr.value(i)).collect();
                    make_typed(PyArray1::from_vec(py, vals).into_any(), Some("int64"))
                } else {
                    make_typed(self.to_py_list(py)?.into_any(), Some("Int64"))
                }
            }
            Self::UInt32(arr) => {
                if arr.null_count() == 0 {
                    let vals: Vec<u32> = (0..arr.len()).map(|i| arr.value(i)).collect();
                    make_typed(PyArray1::from_vec(py, vals).into_any(), Some("uint32"))
                } else {
                    make_typed(self.to_py_list(py)?.into_any(), Some("UInt32"))
                }
            }
            Self::Boolean(arr) => {
                if arr.null_count() == 0 {
                    let vals: Vec<bool> = (0..arr.len()).map(|i| arr.value(i)).collect();
                    make_typed(PyList::new(py, vals)?.into_any(), Some("bool"))
                } else {
                    make_typed(self.to_py_list(py)?.into_any(), Some("boolean"))
                }
            }
            Self::Date32(_) => make_typed(self.to_py_list(py)?.into_any(), None),
            Self::Unsupported(_) => make_typed(self.to_py_list(py)?.into_any(), Some("str")),
        }
    }
}

/// Convert an Arrow [`RecordBatch`] into a `pandas.DataFrame` without requiring `pyarrow`.
///
/// Serves as the fallback when `pyarrow` is not installed so `to_pandas()` on
/// `ArrowTable`, `LabeledSeries`, and result objects only requires `pandas`.
pub(crate) fn record_batch_to_pandas<'py>(
    py: Python<'py>,
    batch: &RecordBatch,
) -> PyResult<Bound<'py, PyAny>> {
    let data = PyDict::new(py);
    let schema = batch.schema();
    for (field, col) in schema.fields().iter().zip(batch.columns().iter()) {
        let view = ColumnView::from_array(col)?;
        data.set_item(field.name(), view.to_pandas_series(py)?)?;
    }
    pandas_dataframe(py)?.call1((data,))
}

/// Convert an Arrow [`RecordBatch`] into a column-oriented `dict[str, list[Any]]`.
pub(crate) fn record_batch_to_py_dict<'py>(
    py: Python<'py>,
    batch: &RecordBatch,
) -> PyResult<Bound<'py, PyDict>> {
    let out = PyDict::new(py);
    let schema = batch.schema();
    for (field, col) in schema.fields().iter().zip(batch.columns().iter()) {
        let view = ColumnView::from_array(col)?;
        out.set_item(field.name(), view.to_py_list(py)?)?;
    }
    Ok(out)
}

/// Convert an Arrow [`RecordBatch`] into a row-oriented `list[dict[str, Any]]`.
pub(crate) fn record_batch_to_py_records<'py>(
    py: Python<'py>,
    batch: &RecordBatch,
) -> PyResult<Bound<'py, PyList>> {
    let num_rows = batch.num_rows();
    if num_rows == 0 {
        return Ok(PyList::empty(py));
    }
    let schema = batch.schema();
    let py_keys: Vec<Bound<'py, PyString>> = schema
        .fields()
        .iter()
        .map(|f| PyString::new(py, f.name()))
        .collect();
    let py_cols: Vec<Bound<'py, PyList>> = batch
        .columns()
        .iter()
        .map(|col| ColumnView::from_array(col)?.to_py_list(py))
        .collect::<PyResult<_>>()?;

    let mut rows = Vec::with_capacity(num_rows);
    for row_idx in 0..num_rows {
        let row = PyDict::new(py);
        for (key, col_list) in py_keys.iter().zip(py_cols.iter()) {
            row.set_item(key, col_list.get_item(row_idx)?)?;
        }
        rows.push(row);
    }
    PyList::new(py, rows)
}

fn escape_html(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#x27;"),
            other => out.push(other),
        }
    }
    out
}

/// Render an Arrow [`RecordBatch`] as an HTML table preview with zero Python dependencies.
pub(crate) fn record_batch_to_html(batch: &RecordBatch) -> String {
    let schema = batch.schema();
    let views: Vec<ColumnView<'_>> = batch
        .columns()
        .iter()
        .filter_map(|c| ColumnView::from_array(c).ok())
        .collect();
    let num_rows = batch.num_rows();
    let num_cols = batch.num_columns();

    let mut html = String::from("<div>\n<table>\n<thead>\n<tr>");
    for field in schema.fields() {
        let _ = write!(html, "<th>{}</th>", escape_html(field.name()));
    }
    html.push_str("</tr>\n</thead>\n<tbody>\n");

    let push_row = |buf: &mut String, row: usize| {
        buf.push_str("<tr>");
        for view in &views {
            let _ = write!(buf, "<td>{}</td>", escape_html(&view.format_cell(row)));
        }
        buf.push_str("</tr>\n");
    };

    if views.len() == num_cols {
        if num_rows > 25 {
            for row in 0..10 {
                push_row(&mut html, row);
            }
            html.push_str("<tr>");
            for _ in 0..num_cols {
                html.push_str("<td>...</td>");
            }
            html.push_str("</tr>\n");
            for row in (num_rows - 10)..num_rows {
                push_row(&mut html, row);
            }
        } else {
            for row in 0..num_rows {
                push_row(&mut html, row);
            }
        }
    }

    let _ = write!(
        html,
        "</tbody>\n</table>\n<p>{num_rows} rows &times; {num_cols} columns</p>\n</div>"
    );
    html
}
