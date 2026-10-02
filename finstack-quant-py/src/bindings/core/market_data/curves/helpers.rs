//! Shared curve binding helpers.

use finstack_quant_core::dates::DayCount;
use finstack_quant_core::market_data::surfaces::{
    VolInterpolationMode, VolQuoteType, VolSurfaceAxis,
};
use finstack_quant_core::market_data::term_structures::{ParInterp, PriceCurveKind, Seniority};
use finstack_quant_core::math::interp::{ExtrapolationPolicy, InterpStyle};
use finstack_quant_core::math::Compounding;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyFloat, PyInt};

use crate::bindings::date_utils::py_to_date;
use crate::bindings::pandas_utils::dict_to_dataframe;

/// Parse a DayCount from a Python string like `"act_365f"`, `"act_360"`, etc.
pub(crate) fn parse_day_count(s: &str) -> PyResult<DayCount> {
    s.parse::<DayCount>()
        .map_err(|e| crate::errors::value_error(format!("Invalid day_count {s:?}: {e}")))
}

/// Parse an [`InterpStyle`] from a Python string.
pub(super) fn parse_interp_style(s: &str) -> PyResult<InterpStyle> {
    s.parse::<InterpStyle>()
        .map_err(|e| crate::errors::value_error(format!("Invalid interp style {s:?}: {e}")))
}

/// Parse an [`ExtrapolationPolicy`] from a Python string.
pub(super) fn parse_extrapolation(s: &str) -> PyResult<ExtrapolationPolicy> {
    s.parse::<ExtrapolationPolicy>()
        .map_err(|e| crate::errors::value_error(format!("Invalid extrapolation {s:?}: {e}")))
}

/// Parse a [`Compounding`] convention from its label
/// (`"continuous"`, `"simple"`, `"annual"`, `"semi_annual"`, `"quarterly"`, `"monthly"`).
pub(super) fn parse_compounding(s: &str) -> PyResult<Compounding> {
    s.parse::<Compounding>().map_err(|_| {
        crate::errors::value_error(format!(
            "Invalid compounding {s:?}: expected one of continuous, simple, annual, semi_annual, quarterly, monthly"
        ))
    })
}

/// Parse a price-curve kind label (Rust `PriceCurveKind::from_str`).
pub(super) fn parse_price_curve_kind(s: &str) -> PyResult<PriceCurveKind> {
    s.parse::<PriceCurveKind>()
        .map_err(crate::errors::core_to_py)
}

/// Canonical label of a price-curve kind (Rust `PriceCurveKind::as_str`).
pub(super) fn price_curve_kind_name(kind: PriceCurveKind) -> &'static str {
    kind.as_str()
}

/// Parse a [`ParInterp`] from its serde name (`"linear"` or `"log_linear"`).
pub(super) fn parse_par_interp(s: &str) -> PyResult<ParInterp> {
    finstack_quant_core::wire::serde_parse(s).map_err(crate::errors::core_to_py)
}

/// Serde name of a [`ParInterp`].
pub(super) fn par_interp_name(value: ParInterp) -> PyResult<String> {
    finstack_quant_core::wire::serde_label(&value).map_err(crate::errors::core_to_py)
}

/// Parse a [`Seniority`] from its serde name
/// (`"senior_secured"`, `"senior"`, `"subordinated"`, `"junior"`).
pub(super) fn parse_seniority(s: &str) -> PyResult<Seniority> {
    s.parse::<Seniority>().map_err(|_| {
        crate::errors::value_error(format!(
            "Invalid seniority {s:?}: expected one of senior_secured, senior, subordinated, junior"
        ))
    })
}

/// Parse a [`VolSurfaceAxis`] from its serde name (`"strike"` or `"tenor"`).
pub(super) fn parse_vol_surface_axis(s: &str) -> PyResult<VolSurfaceAxis> {
    finstack_quant_core::wire::serde_parse(s).map_err(crate::errors::core_to_py)
}

/// Parse a [`VolQuoteType`] from a Python string.
pub(super) fn parse_vol_quote_type(s: &str) -> PyResult<VolQuoteType> {
    s.parse::<VolQuoteType>()
        .map_err(crate::errors::value_error)
}

/// Parse a [`VolInterpolationMode`] from its serde name (`"vol"` or `"total_variance"`).
pub(super) fn parse_vol_interpolation_mode(s: &str) -> PyResult<VolInterpolationMode> {
    finstack_quant_core::wire::serde_parse(s).map_err(crate::errors::core_to_py)
}

/// Serde name of a [`VolInterpolationMode`] (`"vol"` or `"total_variance"`).
pub(super) fn vol_interpolation_mode_name(mode: VolInterpolationMode) -> PyResult<String> {
    finstack_quant_core::wire::serde_label(&mode).map_err(crate::errors::core_to_py)
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
    Date(time::Date),
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

/// Generate `to_json` / `from_json` / `__reduce__` / `__eq__` for a wrapper
/// holding `inner: Arc<T>` where `T: Serialize + DeserializeOwned`.
///
/// Curves and surfaces do not derive `PartialEq` in Rust, so equality compares
/// the canonical JSON wire form, which is exactly what round-trips through
/// `to_json` / `from_json` and `pickle`.
macro_rules! impl_arc_serde_pymethods {
    ($py_ty:ident, $rust_ty:ty, $name:literal) => {
        #[pymethods]
        impl $py_ty {
            #[doc = concat!(
                "Serialize this ", $name, " to its canonical JSON wire form.\n\n",
                "Returns\n-------\nstr\n    Compact JSON that ``from_json`` and the Rust serde impl accept.\n\n",
                "Raises\n------\nValueError\n    If the value cannot be serialized."
            )]
            fn to_json(&self) -> PyResult<String> {
                serde_json::to_string(&*self.inner).map_err(|e| {
                    crate::errors::value_error(format!(concat!("failed to serialize ", $name, ": {}"), e))
                })
            }

            #[doc = concat!(
                "Deserialize a ", $name, " from its canonical JSON wire form.\n\n",
                "Parameters\n----------\njson : str\n    JSON produced by ``to_json`` (or the Rust serde impl).\n\n",
                "Returns\n-------\n", $name, "\n\n",
                "Raises\n------\nValueError\n    If the JSON is malformed, has unknown fields, or fails validation."
            )]
            #[staticmethod]
            fn from_json(json: &str) -> PyResult<Self> {
                serde_json::from_str::<$rust_ty>(json)
                    .map(|value| Self {
                        inner: std::sync::Arc::new(value),
                    })
                    .map_err(|e| {
                        crate::errors::value_error(format!(concat!("invalid ", $name, " JSON: {}"), e))
                    })
            }

            /// Support ``pickle`` (and therefore ``multiprocessing``, ``joblib``, ``dask``)
            /// through the same strict serde round-trip as ``to_json`` / ``from_json``.
            fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
                let from_json = py.get_type::<Self>().getattr("from_json")?;
                crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
            }

            /// Structural equality via the canonical JSON wire form.
            fn __eq__(&self, other: &Bound<'_, PyAny>) -> PyResult<bool> {
                let Ok(other) = other.extract::<PyRef<'_, Self>>() else {
                    return Ok(false);
                };
                Ok(self.to_json()? == other.to_json()?)
            }
        }
    };
}
pub(super) use impl_arc_serde_pymethods;

/// Generate `_repr_html_` delegating to the wrapper's `to_dataframe`.
macro_rules! impl_repr_html_via_dataframe {
    ($py_ty:ident) => {
        #[pymethods]
        impl $py_ty {
            /// Render as an HTML table in Jupyter notebooks.
            ///
            /// Delegates to the frame from ``to_dataframe``; returns ``None`` if the
            /// frame cannot be built so IPython falls back to ``__repr__``.
            fn _repr_html_(&self, py: Python<'_>) -> Option<String> {
                let frame = self.to_dataframe(py).ok()?;
                frame.call_method0("_repr_html_").ok()?.extract().ok()
            }
        }
    };
}
pub(super) use impl_repr_html_via_dataframe;
