//! Shared DataFrame, Series, matrix, and Arrow PyCapsule extraction helpers.
//!
//! Lazily detects loaded host libraries via `sys.modules` so callers using only
//! plain Python / pandas never pay to import `polars` or `pyarrow`.

use finstack_quant_core::dates::Date;
use numpy::{
    PyArrayDescrMethods, PyReadonlyArray1, PyReadonlyArray2, PyUntypedArray, PyUntypedArrayMethods,
};
use pyo3::buffer::PyBuffer;
use pyo3::exceptions::{PyKeyError, PyTypeError};
use pyo3::prelude::*;
use pyo3::types::{PyBool, PyBytes, PyDict, PySlice, PyString};

use crate::bindings::date_utils::py_to_date;
use crate::errors::value_error;

/// Decomposed dated panel: chronological dates, column-major values, and labels.
pub(crate) struct DataFramePanel {
    /// Chronological observation dates.
    pub(crate) dates: Vec<Date>,
    /// `columns[series_idx][row_idx]`.
    pub(crate) columns: Vec<Vec<f64>>,
    /// Series column names in declaration order.
    pub(crate) ticker_names: Vec<String>,
}

fn sys_module_get<'py>(py: Python<'py>, name: &str) -> PyResult<Option<Bound<'py, PyAny>>> {
    let module = py
        .import("sys")?
        .getattr("modules")?
        .call_method1("get", (name,))?;
    Ok((!module.is_none()).then_some(module))
}

/// Look up `polars` in `sys.modules` without importing it.
pub(crate) fn polars_module<'py>(py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
    sys_module_get(py, "polars")
}

/// Return the loaded `polars` module or import it when already committed to a Polars path.
pub(crate) fn require_polars_module<'py>(py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
    match polars_module(py)? {
        Some(pl) => Ok(pl),
        None => Ok(py.import("polars")?.into_any()),
    }
}

/// Look up `pandas` in `sys.modules` without importing it.
pub(crate) fn pandas_module<'py>(py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
    sys_module_get(py, "pandas")
}

/// Return true if `obj` is a `polars.DataFrame`.
pub(crate) fn is_polars_dataframe(obj: &Bound<'_, PyAny>) -> PyResult<bool> {
    let Some(pl) = polars_module(obj.py())? else {
        return Ok(false);
    };
    obj.is_instance(&pl.getattr("DataFrame")?)
}

/// Return true if `obj` is a `polars.Series`.
pub(crate) fn is_polars_series(obj: &Bound<'_, PyAny>) -> PyResult<bool> {
    let Some(pl) = polars_module(obj.py())? else {
        return Ok(false);
    };
    obj.is_instance(&pl.getattr("Series")?)
}

/// Return true if `obj` is a `pandas.DataFrame`.
pub(crate) fn is_pandas_dataframe(obj: &Bound<'_, PyAny>) -> PyResult<bool> {
    let Some(pd) = pandas_module(obj.py())? else {
        return Ok(false);
    };
    obj.is_instance(&pd.getattr("DataFrame")?)
}

/// Return true if `obj` is a `pandas.Series`.
pub(crate) fn is_pandas_series(obj: &Bound<'_, PyAny>) -> PyResult<bool> {
    let Some(pd) = pandas_module(obj.py())? else {
        return Ok(false);
    };
    obj.is_instance(&pd.getattr("Series")?)
}

/// Return true for Arrow C-stream producers (`pyarrow.Table`, `ArrowTable`, DuckDB relations)
/// that are neither pandas nor Polars objects.
pub(crate) fn has_arrow_c_stream(obj: &Bound<'_, PyAny>) -> PyResult<bool> {
    if is_polars_dataframe(obj)?
        || is_polars_series(obj)?
        || is_pandas_dataframe(obj)?
        || is_pandas_series(obj)?
    {
        return Ok(false);
    }
    obj.hasattr("__arrow_c_stream__")
}

/// Return true for a `pandas.DataFrame`, `polars.DataFrame`, or Arrow C-stream table.
pub(crate) fn is_dataframe_like(obj: &Bound<'_, PyAny>) -> PyResult<bool> {
    Ok(is_polars_dataframe(obj)? || is_pandas_dataframe(obj)? || has_arrow_c_stream(obj)?)
}

/// Treat `None`, `float::NAN`, `pd.NA`, `pd.NaT`, and `np.datetime64('NaT')` as missing.
pub(crate) fn is_missing_cell(obj: &Bound<'_, PyAny>) -> PyResult<bool> {
    if obj.is_none() || obj.extract::<f64>().is_ok_and(f64::is_nan) {
        return Ok(true);
    }
    let ty = obj.get_type();
    let name = ty.name()?;
    if name == "NAType" || name == "NaTType" {
        return Ok(true);
    }
    if name == "datetime64" {
        return Ok(obj.str()? == "NaT");
    }
    Ok(false)
}

/// Materialize an Arrow C-stream producer (or return an existing `pyarrow.Table`) as a `pyarrow.Table`.
pub(crate) fn arrow_to_pyarrow_table<'py>(
    py: Python<'py>,
    obj: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    if obj.hasattr("to_pylist")? && obj.hasattr("column_names")? {
        return Ok(obj.clone());
    }
    py.import("pyarrow")?.call_method1("table", (obj,))
}

/// Convert a pandas `DataFrame`, Polars `DataFrame`, or Arrow table to a Python list of row dicts.
pub(crate) fn table_to_py_records<'py>(
    py: Python<'py>,
    obj: &Bound<'py, PyAny>,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    if is_polars_dataframe(obj)? {
        return obj.call_method0("to_dicts").map(Some);
    }
    if is_pandas_dataframe(obj)? {
        return obj.call_method1("to_dict", ("records",)).map(Some);
    }
    if has_arrow_c_stream(obj)? {
        let tbl = arrow_to_pyarrow_table(py, obj)?;
        return tbl.call_method0("to_pylist").map(Some);
    }
    Ok(None)
}

/// Extract string column labels from a pandas `DataFrame`.
pub(crate) fn pandas_column_names(df: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    let cols_list = df.getattr("columns")?.call_method0("tolist")?;
    cols_list.extract().map_err(|_| {
        PyTypeError::new_err(
            "Performance requires string column labels (ticker names); the DataFrame \
             columns are not all str — rename them, e.g. df.columns = df.columns.astype(str)",
        )
    })
}

/// Return column names of a pandas `DataFrame`, Polars `DataFrame`, or Arrow table.
pub(crate) fn table_column_names(obj: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    if is_polars_dataframe(obj)? {
        return obj.getattr("columns")?.extract();
    }
    if has_arrow_c_stream(obj)? {
        let tbl = arrow_to_pyarrow_table(obj.py(), obj)?;
        return tbl.getattr("column_names")?.extract();
    }
    pandas_column_names(obj)
}

/// Return one named column of a table-like object as Python objects, or `None` if absent.
pub(crate) fn table_column_to_pylist<'py>(
    obj: &Bound<'py, PyAny>,
    col: &str,
) -> PyResult<Option<Vec<Bound<'py, PyAny>>>> {
    if is_polars_dataframe(obj)? {
        let cols: Vec<String> = obj.getattr("columns")?.extract()?;
        if !cols.iter().any(|c| c == col) {
            return Ok(None);
        }
        return obj
            .get_item(col)?
            .call_method0("to_list")?
            .extract()
            .map(Some);
    }
    if has_arrow_c_stream(obj)? {
        let tbl = arrow_to_pyarrow_table(obj.py(), obj)?;
        let cols: Vec<String> = tbl.getattr("column_names")?.extract()?;
        if !cols.iter().any(|c| c == col) {
            return Ok(None);
        }
        return tbl
            .call_method1("column", (col,))?
            .call_method0("to_pylist")?
            .extract()
            .map(Some);
    }
    let cols = obj.getattr("columns")?;
    if !cols.contains(col)? {
        return Ok(None);
    }
    obj.get_item(col)?
        .call_method0("tolist")?
        .extract()
        .map(Some)
}

fn is_pandas_bool_series(series: &Bound<'_, PyAny>) -> PyResult<bool> {
    let dtype_name = series
        .getattr("dtype")?
        .getattr("name")?
        .extract::<String>();
    Ok(matches!(dtype_name.as_deref(), Ok("bool" | "boolean")))
}

/// Pull a pandas `Series` column out as `Vec<f64>` (zero-copy buffer on `float64`,
/// nullable `Float64`/`Int64` `pd.NA` converted to `NaN`).
pub(crate) fn extract_float64_column(
    series: &Bound<'_, PyAny>,
    col_label: &str,
) -> PyResult<Vec<f64>> {
    if is_pandas_bool_series(series)? {
        return Err(PyTypeError::new_err(format!(
            "Column {col_label:?} has boolean dtype; expected numeric values"
        )));
    }
    let py = series.py();
    let kwargs = PyDict::new(py);
    kwargs.set_item("dtype", "float64")?;
    kwargs.set_item("copy", false)?;
    let array = match series.call_method("to_numpy", (), Some(&kwargs)) {
        Ok(arr) => arr,
        Err(first_err) => {
            let na_kwargs = PyDict::new(py);
            na_kwargs.set_item("dtype", "float64")?;
            na_kwargs.set_item("na_value", f64::NAN)?;
            series
                .call_method("to_numpy", (), Some(&na_kwargs))
                .map_err(|_| {
                    PyTypeError::new_err(format!(
                        "Column {col_label:?} could not be converted to a float64 NumPy array: {first_err}"
                    ))
                })?
        }
    };
    let buffer = PyBuffer::<f64>::get(&array).map_err(|err| {
        value_error(format!(
            "Column {col_label:?} did not expose a contiguous float64 buffer: {err}"
        ))
    })?;
    buffer.to_vec(py).map_err(|err| {
        value_error(format!(
            "Column {col_label:?} could not be read as float64 buffer: {err}"
        ))
    })
}

/// Extract a numeric Polars `Series` as `Vec<f64>`, converting `null` to `NaN`.
pub(crate) fn extract_polars_float64_column(
    series: &Bound<'_, PyAny>,
    col_label: &str,
) -> PyResult<Vec<f64>> {
    let py = series.py();
    let pl = require_polars_module(py)?;
    let dtype = series.getattr("dtype")?;
    let is_numeric: bool = dtype.call_method0("is_numeric")?.extract()?;
    let is_null: bool = dtype.eq(pl.getattr("Null")?)?;
    if !is_numeric && !is_null {
        return Err(PyTypeError::new_err(format!(
            "Column {col_label:?} has non-numeric Polars dtype {dtype}; expected numeric values"
        )));
    }
    let float_dtype = pl.getattr("Float64")?;
    let float_series = series
        .call_method1("cast", (float_dtype,))
        .map_err(|err| {
            PyTypeError::new_err(format!(
                "Column {col_label:?} could not be converted to Polars Float64: {err}"
            ))
        })?
        .call_method1("fill_null", (f64::NAN,))?;
    let array = float_series.call_method0("to_numpy")?;
    let buffer = PyBuffer::<f64>::get(&array).map_err(|err| {
        value_error(format!(
            "Column {col_label:?} did not expose a float64 buffer: {err}"
        ))
    })?;
    buffer.to_vec(py).map_err(|err| {
        value_error(format!(
            "Column {col_label:?} could not be read as float64 buffer: {err}"
        ))
    })
}

pub(crate) fn extract_arrow_float64_column(
    pa_types: &Bound<'_, PyAny>,
    float64_ty: &Bound<'_, PyAny>,
    col: &Bound<'_, PyAny>,
    col_label: &str,
) -> PyResult<Vec<f64>> {
    let dtype = col.getattr("type")?;
    let is_float: bool = pa_types.call_method1("is_floating", (&dtype,))?.extract()?;
    let is_int: bool = pa_types.call_method1("is_integer", (&dtype,))?.extract()?;
    let is_null: bool = pa_types.call_method1("is_null", (&dtype,))?.extract()?;
    if !is_float && !is_int && !is_null {
        return Err(PyTypeError::new_err(format!(
            "Column {col_label:?} has non-numeric Arrow type {dtype}; expected numeric values"
        )));
    }
    let cast = col.call_method1("cast", (float64_ty,))?;
    let items: Vec<Option<f64>> = cast.call_method0("to_pylist")?.extract()?;
    Ok(items.into_iter().map(|v| v.unwrap_or(f64::NAN)).collect())
}

/// Extract a 1-D float vector from a sequence, NumPy array, pandas `Series`, or Polars `Series`.
pub(crate) fn extract_f64_vec(obj: &Bound<'_, PyAny>, label: &str) -> PyResult<Vec<f64>> {
    if is_polars_series(obj)? {
        return extract_polars_float64_column(obj, label);
    }
    if is_dataframe_like(obj)? {
        return Err(PyTypeError::new_err(format!(
            "{label} must be a sequence of floats, a NumPy array, a pandas Series, or a Polars Series"
        )));
    }
    if is_pandas_series(obj)? {
        return extract_float64_column(obj, label);
    }
    if let Ok(untyped) = obj.cast::<PyUntypedArray>() {
        let kind = untyped.dtype().kind();
        if !matches!(kind, b'f' | b'i' | b'u') {
            return Err(PyTypeError::new_err(format!(
                "{label} must have a real numeric dtype, not bool or non-numeric"
            )));
        }
        if untyped.ndim() != 1 {
            return Err(PyTypeError::new_err(format!(
                "{label} must be a 1-D sequence, got {}-D array",
                untyped.ndim()
            )));
        }
        if let Ok(array) = obj.extract::<PyReadonlyArray1<'_, f64>>() {
            return Ok(array.as_array().iter().copied().collect());
        }
        let casted = obj.call_method1("astype", ("float64",))?;
        let array = casted.extract::<PyReadonlyArray1<'_, f64>>()?;
        return Ok(array.as_array().iter().copied().collect());
    }
    if obj.hasattr("to_numpy")? && obj.hasattr("dtype")? {
        return extract_float64_column(obj, label);
    }
    if obj.is_instance_of::<PyString>() || obj.is_instance_of::<PyBytes>() {
        return Err(PyTypeError::new_err(format!(
            "{label} must be a sequence of floats, a NumPy array, a pandas Series, or a Polars Series"
        )));
    }
    let iter = obj.try_iter().map_err(|_| {
        PyTypeError::new_err(format!(
            "{label} must be a sequence of floats, a NumPy array, a pandas Series, or a Polars Series"
        ))
    })?;
    let mut out = Vec::new();
    for item in iter {
        let item = item?;
        if item.is_instance_of::<PyBool>() {
            return Err(PyTypeError::new_err(format!(
                "{label} elements must be numeric, not bool"
            )));
        }
        out.push(
            item.extract::<f64>().map_err(|_| {
                PyTypeError::new_err(format!("{label} must contain numeric values"))
            })?,
        );
    }
    Ok(out)
}

/// Extract a 1-D optional-float vector (`None`, `pd.NA`, Polars `null`, and `NaN` -> `None`).
pub(crate) fn extract_opt_f64_vec(
    obj: &Bound<'_, PyAny>,
    label: &str,
) -> PyResult<Vec<Option<f64>>> {
    if is_polars_series(obj)? || is_pandas_series(obj)? || obj.is_instance_of::<PyUntypedArray>() {
        return Ok(extract_f64_vec(obj, label)?
            .into_iter()
            .map(|v| (!v.is_nan()).then_some(v))
            .collect());
    }
    if is_dataframe_like(obj)? {
        return Err(PyTypeError::new_err(format!(
            "{label} must be a 1-D sequence, NumPy array, pandas Series, or Polars Series"
        )));
    }
    if obj.is_instance_of::<PyString>() || obj.is_instance_of::<PyBytes>() {
        return Err(PyTypeError::new_err(format!(
            "{label} must be a 1-D sequence of optional floats"
        )));
    }
    let iter = obj.try_iter().map_err(|_| {
        PyTypeError::new_err(format!("{label} must be a 1-D sequence of optional floats"))
    })?;
    let mut out = Vec::new();
    for item in iter {
        let item = item?;
        if is_missing_cell(&item)? {
            out.push(None);
            continue;
        }
        if item.is_instance_of::<PyBool>() {
            return Err(PyTypeError::new_err(format!(
                "{label} elements must be numeric or None, not bool"
            )));
        }
        let val: f64 = item.extract().map_err(|_| {
            PyTypeError::new_err(format!("{label} elements must be numeric or None"))
        })?;
        out.push((!val.is_nan()).then_some(val));
    }
    Ok(out)
}

fn transpose_cols_to_row_major(cols: &[Vec<f64>]) -> (Vec<f64>, usize, usize) {
    let ncols = cols.len();
    let nrows = cols.first().map_or(0, Vec::len);
    let mut out = Vec::with_capacity(nrows * ncols);
    for r in 0..nrows {
        for col in cols {
            out.push(col[r]);
        }
    }
    (out, nrows, ncols)
}

/// Extract a 2-D numeric `(row_major_flat, nrows, ncols)` from a DataFrame, Arrow table, or ndarray.
pub(crate) fn extract_2d_f64(
    obj: &Bound<'_, PyAny>,
    label: &str,
) -> PyResult<Option<(Vec<f64>, usize, usize)>> {
    let py = obj.py();
    if is_polars_dataframe(obj)? {
        let col_names: Vec<String> = obj.getattr("columns")?.extract()?;
        let mut cols = Vec::with_capacity(col_names.len());
        for col in &col_names {
            cols.push(extract_polars_float64_column(&obj.get_item(col)?, col)?);
        }
        return Ok(Some(transpose_cols_to_row_major(&cols)));
    }
    if is_pandas_dataframe(obj)? {
        let nrows: usize = obj.getattr("shape")?.get_item(0)?.extract()?;
        let col_objs: Vec<Bound<'_, PyAny>> =
            obj.getattr("columns")?.call_method0("tolist")?.extract()?;
        if col_objs.is_empty() {
            return Ok(Some((Vec::new(), nrows, 0)));
        }
        let iloc = obj.getattr("iloc")?;
        let all_rows = PySlice::full(py);
        let mut cols = Vec::with_capacity(col_objs.len());
        for (i, col_obj) in col_objs.iter().enumerate() {
            let label = col_obj.str()?.extract::<String>()?;
            let series = iloc.get_item((&all_rows, i))?;
            cols.push(extract_float64_column(&series, &label)?);
        }
        return Ok(Some(transpose_cols_to_row_major(&cols)));
    }
    if has_arrow_c_stream(obj)? {
        let tbl = arrow_to_pyarrow_table(py, obj)?;
        let col_names: Vec<String> = tbl.getattr("column_names")?.extract()?;
        let pa = py.import("pyarrow")?;
        let pa_types = pa.getattr("types")?;
        let float64_ty = pa.getattr("float64")?.call0()?;
        let mut cols = Vec::with_capacity(col_names.len());
        for col in &col_names {
            cols.push(extract_arrow_float64_column(
                &pa_types,
                &float64_ty,
                &tbl.call_method1("column", (col,))?,
                col,
            )?);
        }
        return Ok(Some(transpose_cols_to_row_major(&cols)));
    }
    if let Ok(untyped) = obj.cast::<PyUntypedArray>() {
        let kind = untyped.dtype().kind();
        if !matches!(kind, b'f' | b'i' | b'u') {
            return Err(PyTypeError::new_err(format!(
                "{label} elements must have a real numeric dtype, not bool or non-numeric"
            )));
        }
        if untyped.ndim() != 2 {
            return Err(value_error(
                "expected a rectangular nested list or 2-D array",
            ));
        }
        if let Ok(array) = obj.extract::<PyReadonlyArray2<'_, f64>>() {
            let shape = array.shape();
            return Ok(Some((
                crate::bindings::matrix_input::numpy_row_major(&array),
                shape[0],
                shape[1],
            )));
        }
        let casted = obj.call_method1("astype", ("float64",))?;
        let array = casted.extract::<PyReadonlyArray2<'_, f64>>()?;
        let shape = array.shape();
        return Ok(Some((
            crate::bindings::matrix_input::numpy_row_major(&array),
            shape[0],
            shape[1],
        )));
    }
    Ok(None)
}

/// Resolve an explicit pandas date column index, rejecting missing or duplicate names.
pub(crate) fn resolve_pandas_date_col_idx(col_names: &[String], date_col: &str) -> PyResult<usize> {
    let mut matches = col_names
        .iter()
        .enumerate()
        .filter_map(|(idx, col)| (col == date_col).then_some(idx));
    let Some(first) = matches.next() else {
        return Err(PyKeyError::new_err(format!(
            "date_col {date_col:?} not found in DataFrame columns"
        )));
    };
    if matches.next().is_some() {
        return Err(value_error(format!(
            "duplicate date_col {date_col:?} in DataFrame columns"
        )));
    }
    Ok(first)
}

/// Return true only for `pl.Date` or `pl.Datetime` (excluding `Time`/`Duration`).
pub(crate) fn is_polars_date_dtype(
    pl: &Bound<'_, PyAny>,
    dtype: &Bound<'_, PyAny>,
) -> PyResult<bool> {
    let pl_date = pl.getattr("Date")?;
    let pl_datetime = pl.getattr("Datetime")?;
    Ok(dtype.eq(&pl_date)? || dtype.is_instance(&pl_datetime)? || dtype.eq(&pl_datetime)?)
}

/// Resolve a Polars date column either explicitly (`KeyError` if missing) or by unique `Date`/`Datetime`.
pub(crate) fn resolve_polars_date_col(
    pl: &Bound<'_, PyAny>,
    df: &Bound<'_, PyAny>,
    col_names: &[String],
    date_col: Option<&str>,
) -> PyResult<String> {
    if let Some(name) = date_col {
        if col_names.iter().any(|col| col == name) {
            return Ok(name.to_owned());
        }
        return Err(PyKeyError::new_err(format!(
            "date_col {name:?} not found in DataFrame columns"
        )));
    }
    let mut matches = Vec::new();
    for col in col_names {
        let dtype = df.get_item(col)?.getattr("dtype")?;
        if is_polars_date_dtype(pl, &dtype)? {
            matches.push(col.as_str());
        }
    }
    match matches.as_slice() {
        [single] => Ok((*single).to_owned()),
        [] => Err(value_error(
            "Polars DataFrame has no Date or Datetime column; pass date_col=... to specify the date column",
        )),
        _ => Err(value_error(format!(
            "Polars DataFrame has multiple Date/Datetime columns ({}); keep only the observation date column and numeric series columns",
            matches.join(", ")
        ))),
    }
}

pub(crate) fn resolve_arrow_date_col(
    pa_types: &Bound<'_, PyAny>,
    tbl: &Bound<'_, PyAny>,
    col_names: &[String],
    date_col: Option<&str>,
) -> PyResult<String> {
    if let Some(name) = date_col {
        if col_names.iter().any(|col| col == name) {
            return Ok(name.to_owned());
        }
        return Err(PyKeyError::new_err(format!(
            "date_col {name:?} not found in DataFrame columns"
        )));
    }
    let mut matches = Vec::new();
    for col in col_names {
        let dtype = tbl.call_method1("column", (col,))?.getattr("type")?;
        let is_d: bool = pa_types.call_method1("is_date", (&dtype,))?.extract()?;
        let is_ts: bool = pa_types
            .call_method1("is_timestamp", (&dtype,))?
            .extract()?;
        if is_d || is_ts {
            matches.push(col.as_str());
        }
    }
    match matches.as_slice() {
        [single] => Ok((*single).to_owned()),
        [] => Err(value_error(
            "Arrow table has no Date or Timestamp column; pass date_col=... to specify the date column",
        )),
        _ => Err(value_error(format!(
            "Arrow table has multiple Date/Timestamp columns ({}); keep only the observation date column and numeric series columns",
            matches.join(", ")
        ))),
    }
}

pub(crate) fn extract_polars_dates(series: &Bound<'_, PyAny>) -> PyResult<Vec<Date>> {
    let dates_py: Vec<Bound<'_, PyAny>> = series.call_method0("to_list")?.extract()?;
    dates_py.iter().map(py_to_date).collect()
}

pub(crate) fn extract_pandas_panel(
    df: &Bound<'_, PyAny>,
    date_col: Option<&str>,
) -> PyResult<DataFramePanel> {
    let py = df.py();
    let col_names = pandas_column_names(df)?;
    let iloc = df.getattr("iloc")?;
    let all_rows = PySlice::full(py);

    let (dates_list, date_idx) = if let Some(name) = date_col {
        let idx = resolve_pandas_date_col_idx(&col_names, name)?;
        let series = iloc.get_item((&all_rows, idx))?;
        (series.call_method0("tolist")?, Some(idx))
    } else {
        (df.getattr("index")?.call_method0("tolist")?, None)
    };

    let dates_py: Vec<Bound<'_, PyAny>> = dates_list.extract()?;
    let dates = dates_py
        .iter()
        .map(py_to_date)
        .collect::<PyResult<Vec<_>>>()?;

    let mut ticker_names = Vec::with_capacity(col_names.len());
    let mut columns = Vec::with_capacity(col_names.len());
    for (i, col) in col_names.iter().enumerate() {
        if date_idx == Some(i) {
            continue;
        }
        let series = iloc.get_item((&all_rows, i))?;
        ticker_names.push(col.clone());
        columns.push(extract_float64_column(&series, col)?);
    }

    Ok(DataFramePanel {
        dates,
        columns,
        ticker_names,
    })
}

fn extract_polars_panel(df: &Bound<'_, PyAny>, date_col: Option<&str>) -> PyResult<DataFramePanel> {
    let py = df.py();
    let pl = require_polars_module(py)?;
    let col_names: Vec<String> = df.getattr("columns")?.extract()?;
    let date_name = resolve_polars_date_col(&pl, df, &col_names, date_col)?;
    let dates = extract_polars_dates(&df.get_item(&date_name)?)?;

    let mut ticker_names = Vec::with_capacity(col_names.len().saturating_sub(1));
    let mut columns = Vec::with_capacity(col_names.len().saturating_sub(1));
    for col in &col_names {
        if col == &date_name {
            continue;
        }
        ticker_names.push(col.clone());
        columns.push(extract_polars_float64_column(&df.get_item(col)?, col)?);
    }

    Ok(DataFramePanel {
        dates,
        columns,
        ticker_names,
    })
}

fn extract_arrow_panel(obj: &Bound<'_, PyAny>, date_col: Option<&str>) -> PyResult<DataFramePanel> {
    let py = obj.py();
    let tbl = arrow_to_pyarrow_table(py, obj)?;
    let col_names: Vec<String> = tbl.getattr("column_names")?.extract()?;
    let pa = py.import("pyarrow")?;
    let pa_types = pa.getattr("types")?;
    let float64_ty = pa.getattr("float64")?.call0()?;
    let date_name = resolve_arrow_date_col(&pa_types, &tbl, &col_names, date_col)?;
    let dates_py: Vec<Bound<'_, PyAny>> = tbl
        .call_method1("column", (&date_name,))?
        .call_method0("to_pylist")?
        .extract()?;
    let dates = dates_py
        .iter()
        .map(py_to_date)
        .collect::<PyResult<Vec<_>>>()?;

    let mut ticker_names = Vec::with_capacity(col_names.len().saturating_sub(1));
    let mut columns = Vec::with_capacity(col_names.len().saturating_sub(1));
    for col in &col_names {
        if col == &date_name {
            continue;
        }
        ticker_names.push(col.clone());
        columns.push(extract_arrow_float64_column(
            &pa_types,
            &float64_ty,
            &tbl.call_method1("column", (col,))?,
            col,
        )?);
    }

    Ok(DataFramePanel {
        dates,
        columns,
        ticker_names,
    })
}

fn series_to_frame<'py>(value: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let name = value.getattr("name")?;
    let label: String = if name.is_none() {
        "asset".to_owned()
    } else {
        name.str()?.extract()?
    };
    value.call_method1("to_frame", (label,))
}

/// Extract dates, column-major numeric series, and labels from pandas, Polars, or Arrow tables.
pub(crate) fn extract_dataframe_panel(
    frame: &Bound<'_, PyAny>,
    date_col: Option<&str>,
    error_message: &str,
) -> PyResult<DataFramePanel> {
    if is_polars_dataframe(frame)? {
        return extract_polars_panel(frame, date_col);
    }
    if is_polars_series(frame)? {
        return Err(PyTypeError::new_err(
            "Expected a pandas DataFrame/Series, Polars DataFrame, or Arrow table; a Polars Series has no date index — wrap it in a polars.DataFrame with a date column",
        ));
    }
    if has_arrow_c_stream(frame)? {
        return extract_arrow_panel(frame, date_col);
    }
    let pd = frame.py().import("pandas")?;
    if frame.is_instance(&pd.getattr("Series")?)? {
        if date_col.is_some() {
            return Err(value_error(
                "date_col cannot be used with a pandas Series; a Series always uses its index for dates",
            ));
        }
        let promoted = series_to_frame(frame)?;
        return extract_pandas_panel(&promoted, None);
    }
    if frame.is_instance(&pd.getattr("DataFrame")?)? {
        return extract_pandas_panel(frame, date_col);
    }
    Err(PyTypeError::new_err(error_message.to_owned()))
}
