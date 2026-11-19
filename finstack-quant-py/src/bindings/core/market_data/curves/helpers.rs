//! Shared curve binding helpers.

use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::math::interp::{ExtrapolationPolicy, InterpStyle};
use pyo3::exceptions::PyTypeError;
use pyo3::prelude::*;
use pyo3::types::{PyBool, PyDict, PyFloat, PyInt, PyString};

use crate::bindings::core::dates::daycount::extract_day_count;
use crate::bindings::date_utils::py_to_date;
use crate::bindings::pandas_utils::dict_to_dataframe;
use crate::errors::core_to_py;

/// Shared host fields for a dated curve built from time/value pillars.
pub(super) struct CurveSourceOptions {
    pub(super) id: String,
    pub(super) base_date: Date,
    pub(super) knots: Vec<(f64, f64)>,
    pub(super) day_count: DayCount,
    pub(super) calendar_id: Option<String>,
    pub(super) interp: InterpStyle,
    pub(super) extrapolation: ExtrapolationPolicy,
}

/// Extract a required option and identify the caller when it is absent.
pub(super) fn required_option<'py>(
    options: &Bound<'py, PyDict>,
    label: &str,
    key: &str,
) -> PyResult<Bound<'py, PyAny>> {
    options
        .get_item(key)?
        .ok_or_else(|| PyTypeError::new_err(format!("missing {label} option '{key}'")))
}

impl CurveSourceOptions {
    /// Decode common source fields and reject unknown keys before Rust construction.
    pub(super) fn extract(options: &Bound<'_, PyDict>, label: &str, extra: &str) -> PyResult<Self> {
        for (key, _) in options.iter() {
            let name = key.cast::<PyString>()?.to_str()?;
            if !matches!(
                name,
                "id" | "base_date"
                    | "knots"
                    | "day_count"
                    | "calendar_id"
                    | "interp"
                    | "extrapolation"
            ) && name != extra
            {
                return Err(PyTypeError::new_err(format!(
                    "unknown {label} option '{name}'"
                )));
            }
        }
        let required = |key: &str| required_option(options, label, key);
        let id = required("id")?.extract()?;
        let base_date = py_to_date(&required("base_date")?)?;
        let knots = required("knots")?.extract()?;
        let interp: String = required("interp")?.extract()?;
        let extrapolation: String = required("extrapolation")?.extract()?;
        Ok(Self {
            id,
            base_date,
            knots,
            day_count: extract_day_count(&required("day_count")?)?,
            calendar_id: options
                .get_item("calendar_id")?
                .filter(|v| !v.is_none())
                .map(|v| v.extract())
                .transpose()?,
            interp: interp.parse().map_err(core_to_py)?,
            extrapolation: extrapolation.parse().map_err(core_to_py)?,
        })
    }
}

/// A curve query coordinate: a year fraction or a calendar date.
///
/// Curve query methods accept either form so callers can pass the object they
/// already hold. Year fractions dispatch to the `t`-based Rust method and
/// dates to the `*_on_date` twin.
pub(super) enum TimePoint {
    /// Year fraction from the curve base date.
    Years(f64),
    /// Calendar date, converted with the curve day count by Rust.
    Date(finstack_quant_core::dates::Date),
}

/// Extract a [`TimePoint`] from a `float`/`int` (year fraction, excluding
/// `bool`), a `datetime.date`-like object, or an ISO-8601 date string.
pub(super) fn extract_time_point(obj: &Bound<'_, PyAny>) -> PyResult<TimePoint> {
    if !obj.is_instance_of::<PyBool>()
        && (obj.is_instance_of::<PyFloat>() || obj.is_instance_of::<PyInt>())
    {
        return Ok(TimePoint::Years(obj.extract::<f64>()?));
    }
    Ok(TimePoint::Date(py_to_date(obj)?))
}

/// Build a pandas `DataFrame` from named float columns, in the given order.
pub(super) fn columns_to_dataframe<'py>(
    py: Python<'py>,
    columns: &[(&str, Vec<f64>)],
) -> PyResult<Bound<'py, PyAny>> {
    let data = PyDict::new(py);
    for (name, values) in columns {
        data.set_item(*name, values.clone())?;
    }
    dict_to_dataframe(py, &data, None)
}
