//! Shared helpers for constructing pandas DataFrames and Series from Rust data.

use crate::bindings::date_utils::date_to_py;
use finstack_quant_core::table::{TableColumn, TableColumnData, TableEnvelope};
use numpy::PyArray1;
use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyDict, PyList};
use serde::Serialize;

static PANDAS_DATAFRAME: PyOnceLock<Py<PyAny>> = PyOnceLock::new();

fn pandas_dataframe<'py>(py: Python<'py>) -> PyResult<&'py Bound<'py, PyAny>> {
    PANDAS_DATAFRAME
        .get_or_try_init(py, || {
            Ok(py.import("pandas")?.getattr("DataFrame")?.unbind())
        })
        .map(|ctor| ctor.bind(py))
}

static PANDAS_SERIES: PyOnceLock<Py<PyAny>> = PyOnceLock::new();

fn pandas_series<'py>(py: Python<'py>) -> PyResult<&'py Bound<'py, PyAny>> {
    PANDAS_SERIES
        .get_or_try_init(py, || Ok(py.import("pandas")?.getattr("Series")?.unbind()))
        .map(|ctor| ctor.bind(py))
}

/// Build a `pd.DataFrame` from a dict of column data with an optional index.
///
/// `columns` is a pre-populated `PyDict` mapping column names to list-like values.
/// If `index` is `Some`, it is passed as the `index=` keyword argument.
pub fn dict_to_dataframe<'py>(
    py: Python<'py>,
    columns: &Bound<'py, PyDict>,
    index: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let kwargs = PyDict::new(py);
    if let Some(idx) = index {
        kwargs.set_item("index", idx)?;
    }
    pandas_dataframe(py)?.call((columns,), Some(&kwargs))
}

/// Build a `pd.Series` from already-converted data plus a label index.
///
/// Shared by [`values_to_series`] and [`int_values_to_series`] so both dtypes
/// reach pandas through the same `pd.Series(data, index=..., name=...)` call.
fn build_series<'py>(
    py: Python<'py>,
    data: Bound<'py, PyAny>,
    index: &[String],
    name: &str,
) -> PyResult<Bound<'py, PyAny>> {
    let labels: Vec<&str> = index.iter().map(String::as_str).collect();
    let kwargs = PyDict::new(py);
    kwargs.set_item("index", labels)?;
    kwargs.set_item("name", name)?;
    pandas_series(py)?.call((data,), Some(&kwargs))
}

/// Build a float64 `pd.Series` from per-entity values labelled by `index`.
///
/// Scalar-per-entity metrics return a labelled Series rather than a bare list
/// so callers select by name (`s["FUND"]`) instead of having to remember the
/// positional order, and so `name` survives as the column label under
/// `pd.concat([...], axis=1)`. The data goes through `PyArray1::from_vec`, so
/// there is no per-element `PyFloat` boxing.
pub fn values_to_series<'py>(
    py: Python<'py>,
    values: Vec<f64>,
    index: &[String],
    name: &str,
) -> PyResult<Bound<'py, PyAny>> {
    build_series(py, PyArray1::from_vec(py, values).into_any(), index, name)
}

/// Build an int64 `pd.Series` from per-entity values labelled by `index`.
///
/// The integer counterpart of [`values_to_series`], so integer-valued metrics
/// (drawdown durations in calendar days) keep an integer dtype instead of
/// being silently widened to float64.
pub fn int_values_to_series<'py>(
    py: Python<'py>,
    values: Vec<i64>,
    index: &[String],
    name: &str,
) -> PyResult<Bound<'py, PyAny>> {
    build_series(py, PyArray1::from_vec(py, values).into_any(), index, name)
}

/// Build a single-row pandas DataFrame from any serializable object.
pub fn serde_object_to_single_row_dataframe<'py, T>(
    py: Python<'py>,
    value: &T,
) -> PyResult<Bound<'py, PyAny>>
where
    T: Serialize,
{
    let json = serde_json::to_string(value).map_err(crate::errors::display_to_py)?;
    let row = py.import("json")?.call_method1("loads", (json,))?;
    let rows = PyList::new(py, [row])?;
    pandas_dataframe(py)?.call1((rows,))
}

/// Build a many-row pandas DataFrame from a slice of serializable rows.
///
/// Each row is independently serialized to JSON; pandas infers the column
/// schema from the union of keys. An empty input yields an empty DataFrame.
/// Use this for long-format detail exports where each row is a `(kind, key,
/// amount, ...)` record.
pub fn serde_rows_to_dataframe<'py, T>(py: Python<'py>, rows: &[T]) -> PyResult<Bound<'py, PyAny>>
where
    T: Serialize,
{
    let json = serde_json::to_string(rows).map_err(crate::errors::display_to_py)?;
    let py_rows = py.import("json")?.call_method1("loads", (json,))?;
    pandas_dataframe(py)?.call1((py_rows,))
}

/// Like [`serde_rows_to_dataframe`], but the declared `columns` schema is
/// authoritative: the frame always carries exactly those columns, in that order,
/// whether or not there are rows.
///
/// Two problems make this necessary. `pd.DataFrame([])` has NO columns, so
/// pipelines filtering the documented columns (`df.query("kind.str.startswith('rates')")`)
/// raise on instruments without detail blocks — exactly the heterogeneous-portfolio
/// case the long format exists for (quant review MO-B2). And rows built with
/// `serde_json::json!` are `serde_json::Value::Object`, i.e. a `BTreeMap` unless
/// the `preserve_order` feature is on, so pandas would infer columns in
/// ALPHABETICAL order — silently disagreeing both with the documented `Columns:`
/// list and with the empty-frame layout above.
/// A column's name paired with the pandas dtype an empty frame should carry.
///
/// Use `"float64"`, `"int64"`, `"bool"`, or `"str"`. `"str"` matches what pandas
/// infers for a populated text column on both pandas 2 (object) and pandas 3
/// (`str`), so the empty and populated frames agree either way.
pub type ColumnSchema<'a> = (&'a str, &'a str);
pub fn serde_rows_to_dataframe_with_schema<'py, T>(
    py: Python<'py>,
    rows: &[T],
    columns: &[ColumnSchema<'_>],
) -> PyResult<Bound<'py, PyAny>>
where
    T: Serialize,
{
    let names: Vec<&str> = columns.iter().map(|(name, _)| *name).collect();
    if rows.is_empty() {
        return empty_typed_frame(py, columns);
    }
    let frame = serde_rows_to_dataframe(py, rows)?;
    reindex_columns(py, frame, &names)
}

/// Build a zero-row frame whose columns already carry their real dtypes.
///
/// `pd.DataFrame([], columns=[...])` types every column `object`. Concatenating
/// such a frame with a populated one downgrades EVERY column to `object`, so a
/// numeric column stops being numeric — `groupby().sum()`, arithmetic and
/// `to_parquet` all break, silently. Building from typed empty Series instead
/// means `pd.concat` over a mix of empty and populated results preserves dtypes,
/// which is the whole point of guaranteeing the schema on the empty path.
fn empty_typed_frame<'py>(
    py: Python<'py>,
    columns: &[ColumnSchema<'_>],
) -> PyResult<Bound<'py, PyAny>> {
    let data = PyDict::new(py);
    let series = pandas_series(py)?;
    for (name, dtype) in columns {
        let kwargs = PyDict::new(py);
        kwargs.set_item("dtype", *dtype)?;
        let empty = PyList::empty(py);
        data.set_item(*name, series.call((empty,), Some(&kwargs))?)?;
    }
    dict_to_dataframe(py, &data, None)
}

/// Order `frame` so the declared `columns` come first, in order.
///
/// Any column the rows produced that is NOT in `columns` is kept, appended
/// after the declared ones in its existing order. That matters for the frames
/// whose schema is only partly fixed — `SensitivityResult.to_dataframe` declares
/// `scenario` but adds one column per perturbed parameter, and a strict reindex
/// would silently delete exactly the data the caller wants.
///
/// A declared column the rows did not produce becomes an all-null column rather
/// than raising, so a row type with a `skip_serializing_if` field keeps its
/// documented schema.
fn reindex_columns<'py>(
    py: Python<'py>,
    frame: Bound<'py, PyAny>,
    columns: &[&str],
) -> PyResult<Bound<'py, PyAny>> {
    let present: Vec<String> = frame
        .getattr(pyo3::intern!(py, "columns"))?
        .try_iter()?
        .map(|c| c?.extract::<String>())
        .collect::<PyResult<_>>()?;
    let mut ordered: Vec<&str> = columns.to_vec();
    ordered.extend(
        present
            .iter()
            .map(String::as_str)
            .filter(|name| !columns.contains(name)),
    );
    let kwargs = PyDict::new(py);
    kwargs.set_item("columns", ordered)?;
    frame.call_method(pyo3::intern!(py, "reindex"), (), Some(&kwargs))
}

/// Build a single-row `pd.DataFrame` with an explicit, ordered column schema.
///
/// The single-row frames are built from `serde_json::json!` literals, whose keys
/// land in a `BTreeMap` and therefore reach pandas alphabetically. Passing the
/// documented column list here keeps the emitted order equal to the order the
/// docstring promises, and makes those two share one source of truth.
pub fn serde_object_to_single_row_dataframe_with_schema<'py, T>(
    py: Python<'py>,
    value: &T,
    columns: &[&str],
) -> PyResult<Bound<'py, PyAny>>
where
    T: Serialize,
{
    let frame = serde_object_to_single_row_dataframe(py, value)?;
    reindex_columns(py, frame, columns)
}

/// Convert a slice of `time::Date` into a Python list suitable for a DataFrame index.
pub fn dates_to_pylist<'py>(
    py: Python<'py>,
    dates: &[time::Date],
) -> PyResult<Vec<Bound<'py, PyAny>>> {
    dates.iter().map(|&d| date_to_py(py, d)).collect()
}

/// Convert a slice of `time::Date` into a pandas `DatetimeIndex`.
pub fn dates_to_datetime_index<'py>(
    py: Python<'py>,
    dates: &[time::Date],
) -> PyResult<Bound<'py, PyAny>> {
    let dates = dates_to_pylist(py, dates)?;
    py.import("pandas")?
        .getattr("DatetimeIndex")?
        .call1((dates,))
}

/// Convert a table column into a Python list suitable for pandas construction.
pub fn table_column_to_pylist<'py>(
    py: Python<'py>,
    column: &TableColumn,
) -> PyResult<Bound<'py, PyAny>> {
    let obj: Bound<'py, PyAny> = match &column.data {
        TableColumnData::String(values) => PyList::new(py, values.iter().cloned())?.into_any(),
        TableColumnData::NullableString(values) => {
            PyList::new(py, values.iter().cloned())?.into_any()
        }
        TableColumnData::Float64(values) => PyArray1::from_vec(py, values.clone()).into_any(),
        TableColumnData::NullableFloat64(values) => PyArray1::from_vec(
            py,
            values
                .iter()
                .map(|value| value.unwrap_or(f64::NAN))
                .collect(),
        )
        .into_any(),
        TableColumnData::UInt32(values) => PyArray1::from_vec(py, values.clone()).into_any(),
        TableColumnData::NullableUInt32(values) => {
            PyList::new(py, values.iter().copied())?.into_any()
        }
        TableColumnData::Int64(values) => PyArray1::from_vec(py, values.clone()).into_any(),
        TableColumnData::NullableInt64(values) => {
            PyList::new(py, values.iter().copied())?.into_any()
        }
    };
    Ok(obj)
}

/// Build a pandas DataFrame from every column in a table envelope.
pub fn table_to_dataframe<'py>(
    py: Python<'py>,
    table: &TableEnvelope,
) -> PyResult<Bound<'py, PyAny>> {
    if table.row_count == 0 {
        let schema: Vec<ColumnSchema<'_>> = table
            .columns
            .iter()
            .map(|column| {
                let dtype = match &column.data {
                    TableColumnData::String(_) | TableColumnData::NullableString(_) => "str",
                    TableColumnData::Float64(_) | TableColumnData::NullableFloat64(_) => "float64",
                    TableColumnData::UInt32(_) => "uint32",
                    TableColumnData::NullableUInt32(_) => "UInt32",
                    TableColumnData::Int64(_) => "int64",
                    TableColumnData::NullableInt64(_) => "Int64",
                };
                (column.name.as_str(), dtype)
            })
            .collect();
        return empty_typed_frame(py, &schema);
    }
    let columns = PyDict::new(py);
    for column in &table.columns {
        columns.set_item(column.name.as_str(), table_column_to_pylist(py, column)?)?;
    }
    dict_to_dataframe(py, &columns, None)
}

/// Build a pandas DataFrame from a selected set of table columns.
///
/// Each tuple is `(source_column_name, pandas_column_name)`.
pub fn selected_table_to_dataframe<'py>(
    py: Python<'py>,
    table: &TableEnvelope,
    selected_columns: &[(&str, &str)],
) -> PyResult<Bound<'py, PyAny>> {
    let columns = PyDict::new(py);
    for (source, target) in selected_columns {
        let column = table.column(source).ok_or_else(|| {
            crate::errors::value_error(format!("missing required table column '{source}'"))
        })?;
        columns.set_item(*target, table_column_to_pylist(py, column)?)?;
    }
    dict_to_dataframe(py, &columns, None)
}

/// Convert any `Serialize` value into a Python object via its canonical JSON
/// shape.
///
/// This is the shared escape hatch behind `to_dict()`-style accessors and
/// typed getters that surface nested serde structures (for example
/// [`finstack_quant_core::config::ResultsMeta`]) without hand-writing a
/// `PyDict` per field.
///
/// # Arguments
///
/// * `value` - Any serde-serializable value.
///
/// # Errors
///
/// Returns a `ValueError` if the value cannot be serialized, or propagates a
/// failure from Python's `json.loads`.
pub fn serde_to_py<'py, T>(py: Python<'py>, value: &T) -> PyResult<Bound<'py, PyAny>>
where
    T: Serialize,
{
    let json = serde_json::to_string(value).map_err(crate::errors::display_to_py)?;
    py.import("json")?.call_method1("loads", (json,))
}

/// Build a `pd.Series` from parallel label/value slices.
///
/// Use for one-dimensional labeled vectors (weights by strategy, contribution
/// by position, level by node). For dated series prefer
/// [`values_to_series`] with [`dates_to_datetime_index`].
///
/// # Arguments
///
/// * `labels` - Index labels; must be the same length as `values`.
/// * `values` - Numeric payload.
/// * `name` - Series name (typically the metric being reported).
///
/// # Errors
///
/// Returns a `ValueError` if `labels` and `values` differ in length.
pub fn labeled_values_to_series<'py>(
    py: Python<'py>,
    labels: &[String],
    values: Vec<f64>,
    name: &str,
) -> PyResult<Bound<'py, PyAny>> {
    if labels.len() != values.len() {
        return Err(pyo3::exceptions::PyValueError::new_err(format!(
            "labels and values must have equal length (got {} and {})",
            labels.len(),
            values.len()
        )));
    }
    values_to_series(py, values, labels, name)
}
