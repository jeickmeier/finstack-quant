//! Discount curve bindings.

use finstack_quant_core::market_data::term_structures::DiscountCurve;

use std::sync::Arc;

use pyo3::prelude::*;

use super::helpers::{parse_day_count, parse_extrapolation, parse_interp_style};
use crate::bindings::core::dates::utils::{date_to_py, py_to_date};
use crate::errors::core_to_py;

// ---------------------------------------------------------------------------
// PyDiscountCurve
// ---------------------------------------------------------------------------

/// Discount factor curve for present-value calculations.
///
/// Wraps [`DiscountCurve`] from `finstack-quant-core`. Constructed via the builder
/// pattern using `(time, df)` knot pairs.
#[pyclass(
    name = "DiscountCurve",
    module = "finstack_quant.core.market_data.curves",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyDiscountCurve {
    /// Shared Rust curve.
    pub(crate) inner: Arc<DiscountCurve>,
}

impl PyDiscountCurve {
    /// Build from an existing `Arc<DiscountCurve>`.
    pub(crate) fn from_inner(inner: Arc<DiscountCurve>) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyDiscountCurve {
    /// Construct a discount curve from knot points.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Unique curve identifier (e.g. ``"USD-OIS"``).
    /// base_date : datetime.date
    ///     Valuation date.
    /// knots : list[tuple[float, float]]
    ///     ``(time_years, discount_factor)`` pairs.
    /// interp : str, optional
    ///     Interpolation style (default ``"monotone_convex"``).
    /// extrapolation : str, optional
    ///     Extrapolation policy (default ``"flat_forward"``).
    /// day_count : str, optional
    ///     Day-count convention. When omitted, Rust infers a market default from the curve ID.
    #[new]
    #[pyo3(signature = (id, base_date, knots, interp="monotone_convex", extrapolation="flat_forward", day_count=None))]
    fn new(
        id: &str,
        base_date: &Bound<'_, PyAny>,
        knots: Vec<(f64, f64)>,
        interp: &str,
        extrapolation: &str,
        day_count: Option<&str>,
    ) -> PyResult<Self> {
        let base = py_to_date(base_date)?;
        let style = parse_interp_style(interp)?;
        let extrap = parse_extrapolation(extrapolation)?;

        let mut builder = DiscountCurve::builder(id)
            .base_date(base)
            .knots(knots)
            .interp(style)
            .extrapolation(extrap);
        if let Some(day_count) = day_count {
            builder = builder.day_count(parse_day_count(day_count)?);
        }

        let curve = builder.build().map_err(core_to_py)?;

        Ok(Self {
            inner: Arc::new(curve),
        })
    }

    /// Discount factor at year fraction `t`.
    #[pyo3(text_signature = "(self, t)")]
    fn df(&self, t: f64) -> f64 {
        self.inner.df(t)
    }

    /// Continuously-compounded zero rate at year fraction `t`.
    #[pyo3(text_signature = "(self, t)")]
    fn zero(&self, t: f64) -> f64 {
        self.inner.zero(t)
    }

    /// Continuously-compounded forward rate between `t1` and `t2`.
    #[pyo3(text_signature = "(self, t1, t2)")]
    fn forward(&self, t1: f64, t2: f64) -> PyResult<f64> {
        self.inner.forward(t1, t2).map_err(core_to_py)
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
        format!("DiscountCurve(id={:?})", self.inner.id().as_str())
    }
}
