//! Shared curve binding helpers.

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyFloat, PyInt};

use crate::bindings::date_utils::py_to_date;
use crate::bindings::pandas_utils::dict_to_dataframe;

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

/// Extract a [`TimePoint`] from a `float`/`int` (year fraction), a
/// `datetime.date`-like object, or an ISO-8601 date string.
pub(super) fn extract_time_point(obj: &Bound<'_, PyAny>) -> PyResult<TimePoint> {
    if obj.is_instance_of::<PyFloat>() || obj.is_instance_of::<PyInt>() {
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
