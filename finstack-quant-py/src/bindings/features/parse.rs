//! Key-column, exposure, and parameter parsing helpers for feature bindings.

use crate::bindings::module_utils::py_to_json_value;
use crate::errors::core_to_py;
use pyo3::prelude::*;
use serde_json::Value;

/// Convert one key-column sequence into strings.
///
/// Strings pass through. Datetimes and pandas timestamps go to the Rust key
/// formatter; ``datetime.date`` values use ``isoformat``. Every other element
/// type (ints, floats, durations such as ``pd.Timedelta``, times) raises
/// ``TypeError``. Aware and naive datetimes must not be mixed.
pub(super) fn extract_keys(obj: &Bound<'_, PyAny>, role: &str) -> PyResult<Vec<String>> {
    let mut keys = Vec::new();
    let datetime_module = obj.py().import("datetime")?;
    let datetime_type = datetime_module.getattr("datetime")?;
    let date_type = datetime_module.getattr("date")?;
    let mut aware_kind = None;
    for item in obj.try_iter().map_err(|_| {
        pyo3::exceptions::PyTypeError::new_err(format!(
            "{role} must be a sequence of str or date-like values"
        ))
    })? {
        let item = item?;
        match item.extract::<String>() {
            Ok(text) => {
                keys.push(text);
            }
            _ => {
                let mut item_obj = item;
                let type_name = item_obj.get_type().name()?;
                if type_name == "NaTType" {
                    return Err(pyo3::exceptions::PyValueError::new_err(format!(
                        "{role} must not contain NaT"
                    )));
                }
                if type_name == "datetime64" {
                    if item_obj.str()? == "NaT" {
                        return Err(pyo3::exceptions::PyValueError::new_err(format!(
                            "{role} must not contain NaT"
                        )));
                    }
                    let py_val = item_obj.call_method0("item")?;
                    item_obj = if py_val.is_instance_of::<pyo3::types::PyInt>() {
                        item_obj
                            .call_method1("astype", ("datetime64[us]",))?
                            .call_method0("item")?
                    } else {
                        py_val
                    };
                }
                let item = item_obj;
                if item.is_instance(&datetime_type)? {
                    let offset = item.call_method0("utcoffset")?;
                    let aware = !offset.is_none();
                    if aware_kind.is_some_and(|previous| previous != aware) {
                        return Err(pyo3::exceptions::PyValueError::new_err(format!(
                            "{role} must not mix timezone-aware and naive datetimes"
                        )));
                    }
                    aware_kind = Some(aware);
                    let wall = wall_datetime(&item)?;
                    keys.push(if aware {
                        let seconds: i32 = offset
                            .call_method0("total_seconds")?
                            .call_method0("__int__")?
                            .extract()?;
                        let offset = time::UtcOffset::from_whole_seconds(seconds)
                            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
                        finstack_quant_features::datetime_order_key(wall.assume_offset(offset))
                    } else {
                        finstack_quant_features::naive_datetime_order_key(wall)
                    });
                } else if item.is_instance(&date_type)? {
                    // A plain date: ISO `YYYY-MM-DD` sorts chronologically. Other
                    // objects with an `isoformat` (durations such as `pd.Timedelta`,
                    // times) do not sort correctly as text, so they are rejected below.
                    keys.push(item.call_method0("isoformat")?.extract()?);
                } else {
                    return Err(pyo3::exceptions::PyTypeError::new_err(format!(
                        "{role} entries must be str, datetime or date, got {}",
                        item.get_type()
                            .name()
                            .map_or_else(|_| "?".to_string(), |n| n.to_string())
                    )));
                }
            }
        }
    }
    Ok(keys)
}

/// Wall-clock fields of a ``datetime`` / pandas ``Timestamp`` (with its
/// ``nanosecond`` when present), without any timezone conversion.
fn wall_datetime(value: &Bound<'_, PyAny>) -> PyResult<time::PrimitiveDateTime> {
    let small = |name: &str| -> PyResult<u8> { value.getattr(name)?.extract() };
    let microsecond: u32 = value.getattr("microsecond")?.extract()?;
    let nanosecond: u32 = if value.hasattr("nanosecond")? {
        value.getattr("nanosecond")?.extract()?
    } else {
        0
    };
    let invalid =
        |e: time::error::ComponentRange| pyo3::exceptions::PyValueError::new_err(e.to_string());
    let month = time::Month::try_from(small("month")?).map_err(invalid)?;
    let year: i32 = value.getattr("year")?.extract()?;
    let date = finstack_quant_core::dates::Date::from_calendar_date(year, month, small("day")?)
        .map_err(invalid)?;
    let clock = time::Time::from_hms_nano(
        small("hour")?,
        small("minute")?,
        small("second")?,
        microsecond * 1000 + nanosecond,
    )
    .map_err(invalid)?;
    Ok(time::PrimitiveDateTime::new(date, clock))
}

pub(super) fn parse_params(
    py: Python<'_>,
    params: Option<&Bound<'_, PyAny>>,
    label: &str,
) -> PyResult<Option<Value>> {
    params
        .map(|value| py_to_json_value(py, value, label))
        .transpose()
}

/// Extract the canonical Rust operation from a ``str`` or its Python wrapper.
pub(super) fn extract_op<T, R>(obj: &Bound<'_, PyAny>, inner_of: impl Fn(&T) -> R) -> PyResult<R>
where
    T: pyo3::PyClass,
    R: std::str::FromStr<Err = finstack_quant_core::Error>,
{
    use pyo3::types::{PyString, PyStringMethods};

    if let Ok(text) = obj.cast::<PyString>() {
        return text.to_str()?.parse().map_err(core_to_py);
    }
    if let Ok(op) = obj.extract::<PyRef<'_, T>>() {
        return Ok(inner_of(&op));
    }
    Err(pyo3::exceptions::PyTypeError::new_err(
        "op must be a str or the corresponding operation enum value",
    ))
}

pub(super) fn extract_opt_f64_vec(
    obj: &Bound<'_, PyAny>,
    role: &str,
) -> PyResult<Vec<Option<f64>>> {
    crate::bindings::dataframe_utils::extract_opt_f64_vec(obj, role)
}

pub(super) fn extract_exposures(obj: &Bound<'_, PyAny>) -> PyResult<Vec<Vec<Option<f64>>>> {
    use crate::bindings::dataframe_utils::{
        arrow_to_pyarrow_table, has_arrow_c_stream, is_pandas_dataframe, is_polars_dataframe,
        table_column_names,
    };
    if is_polars_dataframe(obj)? {
        let cols: Vec<String> = obj.getattr("columns")?.extract()?;
        return cols
            .iter()
            .map(|c| extract_opt_f64_vec(&obj.get_item(c)?, c))
            .collect();
    }
    if is_pandas_dataframe(obj)? {
        let cols = table_column_names(obj)?;
        let iloc = obj.getattr("iloc")?;
        let all_rows = pyo3::types::PySlice::full(obj.py());
        return (0..cols.len())
            .map(|i| extract_opt_f64_vec(&iloc.get_item((&all_rows, i))?, &cols[i]))
            .collect();
    }
    if has_arrow_c_stream(obj)? {
        let tbl = arrow_to_pyarrow_table(obj.py(), obj)?;
        let cols: Vec<String> = tbl.getattr("column_names")?.extract()?;
        let mut out = Vec::with_capacity(cols.len());
        for c in &cols {
            let py_items: Vec<Bound<'_, PyAny>> = tbl
                .call_method1("column", (c,))?
                .call_method0("to_pylist")?
                .extract()?;
            let list = pyo3::types::PyList::new(obj.py(), py_items)?;
            out.push(extract_opt_f64_vec(&list, c)?);
        }
        return Ok(out);
    }
    let iter = obj.try_iter().map_err(|_| {
        pyo3::exceptions::PyTypeError::new_err(
            "exposures must be column-major sequences of optional floats or a DataFrame",
        )
    })?;
    let mut out = Vec::new();
    for item in iter {
        let item = item?;
        out.push(extract_opt_f64_vec(&item, "exposures")?);
    }
    Ok(out)
}
