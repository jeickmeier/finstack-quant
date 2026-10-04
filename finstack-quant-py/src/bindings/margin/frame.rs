//! pandas ``DataFrame`` ingestion helpers shared by the margin bindings.
//!
//! The ``from_dataframe`` constructors accept the long-format frames the
//! matching ``to_dataframe`` exits emit. Rows are pulled through
//! ``DataFrame.to_dict("records")`` so the binding only touches plain Python
//! dicts; all interpretation of the rows happens in Rust (the ``add_*`` adders
//! and the sensitivity ``from_rows`` constructors).

use finstack_quant_margin::table::SensitivityRow;
use pyo3::prelude::*;
use pyo3::types::PyDict;

/// Materialise a ``DataFrame`` (or any object with ``to_dict``) as row dicts.
pub(super) fn records<'py>(frame: &Bound<'py, PyAny>) -> PyResult<Vec<Bound<'py, PyDict>>> {
    let rows = frame.call_method1(pyo3::intern!(frame.py(), "to_dict"), ("records",))?;
    rows.try_iter()?
        .map(|row| row?.cast_into::<PyDict>().map_err(PyErr::from))
        .collect()
}

/// Whether a cell should be treated as missing (``None`` or a float ``NaN``).
fn is_missing(value: &Bound<'_, PyAny>) -> bool {
    value.is_none() || value.extract::<f64>().is_ok_and(f64::is_nan)
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
            if let Ok(number) = value.extract::<f64>() {
                if number.fract() == 0.0 {
                    return Ok(Some(format!("{}", number as i64)));
                }
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
pub(super) fn req_date(row: &Bound<'_, PyDict>, key: &str) -> PyResult<time::Date> {
    match row.get_item(key)? {
        Some(value) if !is_missing(&value) => crate::bindings::date_utils::py_to_date(&value),
        _ => Err(crate::errors::value_error(format!(
            "from_dataframe: column '{key}' is missing or null"
        ))),
    }
}

/// Read an optional float cell; ``None``/``NaN`` become ``None``.
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

/// Convert ``list[tuple[float, float]] | pandas.Series`` into ``(x, y)`` pairs.
///
/// A ``Series`` contributes its index as ``x`` and its values as ``y``; any
/// other iterable must yield two-element tuples.
pub(super) fn pairs_from_series_or_list(obj: &Bound<'_, PyAny>) -> PyResult<Vec<(f64, f64)>> {
    if obj.hasattr("items")? && obj.hasattr("index")? {
        let mut out = Vec::new();
        for item in obj.call_method0("items")?.try_iter()? {
            let (x, y): (f64, f64) = item?.extract()?;
            out.push((x, y));
        }
        return Ok(out);
    }
    obj.extract::<Vec<(f64, f64)>>()
}
