//! Forward curve bindings.

use finstack_core::market_data::term_structures::ForwardCurve;

use std::sync::Arc;

use pyo3::prelude::*;

use super::helpers::{parse_day_count, parse_extrapolation, parse_interp_style};
use crate::bindings::core::dates::utils::{date_to_py, py_to_date};
use crate::errors::core_to_py;

// ---------------------------------------------------------------------------
// PyForwardCurve
// ---------------------------------------------------------------------------

/// Forward rate curve for a floating-rate index with a fixed tenor.
///
/// Wraps [`ForwardCurve`] from `finstack-core`.
#[pyclass(
    name = "ForwardCurve",
    module = "finstack.core.market_data.curves",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyForwardCurve {
    /// Shared Rust curve.
    pub(crate) inner: Arc<ForwardCurve>,
}

impl PyForwardCurve {
    /// Build from an existing `Arc<ForwardCurve>`.
    pub(crate) fn from_inner(inner: Arc<ForwardCurve>) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyForwardCurve {
    /// Construct a forward rate curve from knot points.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Unique curve identifier (e.g. ``"USD-SOFR-3M"``).
    /// tenor : float
    ///     Index tenor in years (e.g. ``0.25`` for 3 months).
    /// knots : list[tuple[float, float]]
    ///     ``(time_years, forward_rate)`` pairs.
    /// base_date : datetime.date
    ///     Valuation date.
    /// day_count : str, optional
    ///     Day-count convention (default ``"act_360"``).
    /// interp : str, optional
    ///     Interpolation style (default ``"linear"``).
    /// extrapolation : str, optional
    ///     Extrapolation policy (default ``"flat_forward"``).
    #[new]
    #[pyo3(signature = (id, tenor, knots, base_date, day_count="act_360", interp="linear", extrapolation="flat_forward"))]
    fn new(
        id: &str,
        tenor: f64,
        knots: Vec<(f64, f64)>,
        base_date: &Bound<'_, PyAny>,
        day_count: &str,
        interp: &str,
        extrapolation: &str,
    ) -> PyResult<Self> {
        let base = py_to_date(base_date)?;
        let dc = parse_day_count(day_count)?;
        let style = parse_interp_style(interp)?;
        let extrap = parse_extrapolation(extrapolation)?;

        let curve = ForwardCurve::builder(id, tenor)
            .base_date(base)
            .day_count(dc)
            .knots(knots)
            .interp(style)
            .extrapolation(extrap)
            .build()
            .map_err(core_to_py)?;

        Ok(Self {
            inner: Arc::new(curve),
        })
    }

    /// Forward rate at year fraction `t`.
    #[pyo3(text_signature = "(self, t)")]
    fn rate(&self, t: f64) -> f64 {
        self.inner.rate(t)
    }

    /// Curve identifier string.
    #[getter]
    fn id(&self) -> &str {
        self.inner.id().as_str()
    }

    /// Valuation base date.
    #[getter]
    fn base_date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.base_date())
    }

    fn __repr__(&self) -> String {
        format!("ForwardCurve(id={:?})", self.inner.id().as_str())
    }
}

// ---------------------------------------------------------------------------
