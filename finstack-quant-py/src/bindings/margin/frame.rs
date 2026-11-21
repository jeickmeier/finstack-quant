//! pandas ``DataFrame`` ingestion helpers shared by the margin bindings.
//!
//! The ``from_dataframe`` constructors accept the long-format frames the
//! matching ``to_pandas`` / ``to_arrow`` exits emit. Rows are pulled through
//! ``DataFrame.to_dict("records")`` so the binding only touches plain Python
//! dicts; all interpretation of the rows happens in Rust (the ``add_*`` adders
//! and the sensitivity ``from_rows`` constructors).

use finstack_quant_margin::table::SensitivityRow;
use pyo3::prelude::*;
use pyo3::types::PyDict;

/// Materialise a pandas `DataFrame`, Polars `DataFrame`, Arrow table, or any
/// object exposing ``to_dict("records")`` as Python row dicts.
pub(super) fn records<'py>(frame: &Bound<'py, PyAny>) -> PyResult<Vec<Bound<'py, PyDict>>> {
    let rows = match crate::bindings::dataframe_utils::table_to_py_records(frame.py(), frame)? {
        Some(rows) => rows,
        None => frame.call_method1(pyo3::intern!(frame.py(), "to_dict"), ("records",))?,
    };
    rows.try_iter()?
        .map(|row| row?.cast_into::<PyDict>().map_err(PyErr::from))
        .collect()
}

/// Whether a cell should be treated as missing (``None``, ``NaN``, ``pd.NA``,
/// ``pd.NaT``, or ``np.datetime64('NaT')``).
fn is_missing(value: &Bound<'_, PyAny>) -> bool {
    crate::bindings::dataframe_utils::is_missing_cell(value).unwrap_or(false)
}

/// Read an optional text cell; ``None``/``NaN`` become ``None``.
///
/// Numeric cells (a bucket index that pandas inferred as ``int64``) are
/// rendered with ``str()`` so a frame round-tripped through ``pd.to_numeric``
/// still ingests.
pub(super) fn opt_str(row: &Bound<'_, PyDict>, key: &str) -> PyResult<Option<String>> {
    match row.get_item(key)? {
        Some(value) if !is_missing(&value) => {
            if let Ok(text) = value.extract::<String>() {
                return Ok(Some(text));
            }
            if let Ok(number) = value.extract::<f64>()
                && number.fract() == 0.0
            {
                return Ok(Some(format!("{}", number as i64)));
            }
            Ok(Some(value.str()?.to_string()))
        }
        _ => Ok(None),
    }
}

/// Read a required text cell.
pub(super) fn req_str(row: &Bound<'_, PyDict>, key: &str) -> PyResult<String> {
    opt_str(row, key)?.ok_or_else(|| {
        crate::errors::value_error(format!("from_dataframe: column '{key}' is missing or null"))
    })
}

/// Read a required float cell.
pub(super) fn req_f64(row: &Bound<'_, PyDict>, key: &str) -> PyResult<f64> {
    match row.get_item(key)? {
        Some(value) if !value.is_none() => value.extract::<f64>().map_err(|_| {
            crate::errors::value_error(format!(
                "from_dataframe: column '{key}' must be numeric, got {}",
                value
                    .get_type()
                    .name()
                    .map(|n| n.to_string())
                    .unwrap_or_default()
            ))
        }),
        _ => Err(crate::errors::value_error(format!(
            "from_dataframe: column '{key}' is missing or null"
        ))),
    }
}

/// Read a required boolean cell.
pub(super) fn req_bool(row: &Bound<'_, PyDict>, key: &str) -> PyResult<bool> {
    match row.get_item(key)? {
        Some(value) if !is_missing(&value) => value.extract::<bool>().map_err(|_| {
            crate::errors::value_error(format!("from_dataframe: column '{key}' must be boolean"))
        }),
        _ => Err(crate::errors::value_error(format!(
            "from_dataframe: column '{key}' is missing or null"
        ))),
    }
}

/// Read a required date-like cell.
pub(super) fn req_date(
    row: &Bound<'_, PyDict>,
    key: &str,
) -> PyResult<finstack_quant_core::dates::Date> {
    match row.get_item(key)? {
        Some(value) if !is_missing(&value) => crate::bindings::date_utils::py_to_date(&value),
        _ => Err(crate::errors::value_error(format!(
            "from_dataframe: column '{key}' is missing or null"
        ))),
    }
}

/// Read an optional date-like cell; missing cells become ``None``.
pub(super) fn opt_date(
    row: &Bound<'_, PyDict>,
    key: &str,
) -> PyResult<Option<finstack_quant_core::dates::Date>> {
    match row.get_item(key)? {
        Some(value) if !is_missing(&value) => {
            crate::bindings::date_utils::py_to_date(&value).map(Some)
        }
        _ => Ok(None),
    }
}

/// Read an optional float cell; ``None``/``NaN``/``pd.NA`` become ``None``.
fn opt_f64(row: &Bound<'_, PyDict>, key: &str) -> PyResult<Option<f64>> {
    match row.get_item(key)? {
        Some(value) if !is_missing(&value) => value.extract::<f64>().map(Some).map_err(|_| {
            crate::errors::value_error(format!("from_dataframe: column '{key}' must be numeric"))
        }),
        _ => Ok(None),
    }
}

/// Convert a long-format sensitivity frame into Rust [`SensitivityRow`]s.
///
/// Only cell conversion happens here; the meaning of each
/// ``(risk_class, kind)`` row is decoded by the Rust ``from_rows``
/// constructors. Absent optional columns read as nulls.
pub(super) fn sensitivity_rows(frame: &Bound<'_, PyAny>) -> PyResult<Vec<SensitivityRow>> {
    records(frame)?
        .iter()
        .map(|row| {
            Ok(SensitivityRow {
                risk_class: req_str(row, "risk_class")?,
                bucket: opt_str(row, "bucket")?,
                tenor: opt_str(row, "tenor")?,
                curve_id: opt_str(row, "curve_id")?,
                expiry_tenor: opt_str(row, "expiry_tenor")?,
                issuer: opt_str(row, "issuer")?,
                kind: req_str(row, "kind")?,
                sector: opt_str(row, "sector")?,
                seniority: opt_str(row, "seniority")?,
                maturity_years: opt_f64(row, "maturity_years")?,
                pnl_adjustment: opt_f64(row, "pnl_adjustment")?,
                amount: req_f64(row, "amount")?,
            })
        })
        .collect()
}

/// Read dated funding observations from explicit pairs, a pandas Series,
/// or a 2-column DataFrame/Arrow table.
pub(super) fn dated_pairs_from_series_or_list(
    observations: &Bound<'_, PyAny>,
) -> PyResult<Vec<(finstack_quant_core::dates::Date, f64)>> {
    use crate::bindings::dataframe_utils::{
        extract_f64_vec, is_dataframe_like, is_polars_dataframe, table_column_names,
        table_column_to_pylist,
    };
    let read_pair = |item: Bound<'_, PyAny>| {
        let (date, spread): (Bound<'_, PyAny>, f64) = item.extract()?;
        Ok((crate::bindings::date_utils::py_to_date(&date)?, spread))
    };
    if observations.hasattr("items")? && observations.hasattr("index")? {
        return observations
            .call_method0("items")?
            .try_iter()?
            .map(|item| item.and_then(read_pair))
            .collect();
    }
    if is_dataframe_like(observations)? {
        let cols = table_column_names(observations)?;
        if cols.len() != 2 {
            return Err(crate::errors::value_error(format!(
                "dated observations DataFrame must have 2 columns (date, value), got {}",
                cols.len()
            )));
        }
        let dates_py = table_column_to_pylist(observations, &cols[0])?.unwrap_or_default();
        let vals = if is_polars_dataframe(observations)? {
            extract_f64_vec(&observations.get_item(&cols[1])?, &cols[1])?
        } else {
            let vals_py = table_column_to_pylist(observations, &cols[1])?.unwrap_or_default();
            vals_py
                .into_iter()
                .map(|v| v.extract::<f64>())
                .collect::<PyResult<Vec<_>>>()?
        };
        return dates_py
            .iter()
            .zip(vals)
            .map(|(d, v)| Ok((crate::bindings::date_utils::py_to_date(d)?, v)))
            .collect();
    }
    observations
        .try_iter()?
        .map(|item| item.and_then(read_pair))
        .collect()
}
