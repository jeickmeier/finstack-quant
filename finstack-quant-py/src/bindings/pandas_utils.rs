//! Shared helpers for constructing pandas DataFrames from Rust data.

use crate::bindings::core::dates::utils::date_to_py;
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

/// Like [`serde_rows_to_dataframe`], but an empty input yields a zero-row
/// DataFrame that still carries the fixed column schema.
///
/// `pd.DataFrame([])` has NO columns, so pipelines filtering the documented
/// columns (`df.query("kind.str.startswith('rates')")`) raise on instruments
/// without detail blocks — exactly the heterogeneous-portfolio case the long
/// format exists for (quant review MO-B2).
pub fn serde_rows_to_dataframe_with_schema<'py, T>(
    py: Python<'py>,
    rows: &[T],
    columns: &[&str],
) -> PyResult<Bound<'py, PyAny>>
where
    T: Serialize,
{
    if rows.is_empty() {
        let kwargs = pyo3::types::PyDict::new(py);
        kwargs.set_item("columns", columns.to_vec())?;
        let empty = PyList::empty(py);
        return pandas_dataframe(py)?.call((empty,), Some(&kwargs));
    }
    serde_rows_to_dataframe(py, rows)
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
        TableColumnData::NullableFloat64(values) => {
            PyList::new(py, values.iter().copied())?.into_any()
        }
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
