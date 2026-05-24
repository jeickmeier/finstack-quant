//! Date parsing helpers shared by Python bindings.

use pyo3::prelude::*;

use crate::errors::display_to_py;

/// Parse an ISO 8601 date string into a `time::Date`.
pub(crate) fn parse_iso_date_py(s: &str) -> PyResult<time::Date> {
    finstack_core::dates::parse_iso_date(s).map_err(display_to_py)
}
