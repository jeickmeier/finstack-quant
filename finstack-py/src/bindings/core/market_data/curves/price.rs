//! Price curve bindings.

use finstack_core::market_data::term_structures::PriceCurve;

use std::sync::Arc;

use pyo3::prelude::*;

use super::helpers::{parse_day_count, parse_extrapolation, parse_interp_style};
use crate::bindings::core::dates::utils::{date_to_py, py_to_date};
use crate::errors::core_to_py;

// PyPriceCurve
// ---------------------------------------------------------------------------

/// Forward price curve for commodities and other price-based assets.
///
/// Wraps [`PriceCurve`] from `finstack-core`.
#[pyclass(
    name = "PriceCurve",
    module = "finstack.core.market_data.curves",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyPriceCurve {
    /// Shared Rust curve.
    pub(crate) inner: Arc<PriceCurve>,
}

impl PyPriceCurve {
    /// Build from an existing `Arc<PriceCurve>`.
    pub(crate) fn from_inner(inner: Arc<PriceCurve>) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyPriceCurve {
    /// Construct a price curve from knot points.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Unique curve identifier (e.g. ``"WTI-FORWARD"``).
    /// base_date : datetime.date
    ///     Valuation date.
    /// knots : list[tuple[float, float]]
    ///     ``(time_years, forward_price)`` pairs.
    /// extrapolation : str, optional
    ///     Extrapolation policy (default ``"flat_zero"``).
    /// interp : str, optional
    ///     Interpolation style (default ``"linear"``).
    /// day_count : str, optional
    ///     Day-count convention (default ``"act_365f"``).
    #[new]
    #[pyo3(signature = (id, base_date, knots, extrapolation="flat_zero", interp="linear", day_count="act_365f"))]
    fn new(
        id: &str,
        base_date: &Bound<'_, PyAny>,
        knots: Vec<(f64, f64)>,
        extrapolation: &str,
        interp: &str,
        day_count: &str,
    ) -> PyResult<Self> {
        let base = py_to_date(base_date)?;
        let extrap = parse_extrapolation(extrapolation)?;
        let style = parse_interp_style(interp)?;
        let dc = parse_day_count(day_count)?;

        let curve = PriceCurve::builder(id)
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

    /// Forward price at year fraction `t`.
    #[pyo3(text_signature = "(self, t)")]
    fn price(&self, t: f64) -> f64 {
        self.inner.price(t)
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
        format!("PriceCurve(id={:?})", self.inner.id().as_str())
    }
}

// ---------------------------------------------------------------------------
