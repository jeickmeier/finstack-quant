//! Forward curve bindings.

use finstack_quant_core::market_data::term_structures::ForwardCurve;

use std::sync::Arc;

use pyo3::prelude::*;
use pyo3::types::PyType;

use super::helpers::{parse_day_count, parse_extrapolation, parse_interp_style};
use crate::bindings::core::dates::utils::{date_to_py, py_to_date};
use crate::errors::core_to_py;

// ---------------------------------------------------------------------------
// PyForwardCurve
// ---------------------------------------------------------------------------

/// Forward rate curve for a floating-rate index with a fixed tenor.
///
/// Wraps [`ForwardCurve`] from `finstack-quant-core`.
#[pyclass(
    name = "ForwardCurve",
    module = "finstack_quant.core.market_data.curves",
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

    #[expect(
        clippy::too_many_arguments,
        reason = "the shared constructor receives the complete curve specification"
    )]
    fn build(
        id: &str,
        tenor: f64,
        base_date: &Bound<'_, PyAny>,
        knots: Vec<(f64, f64)>,
        day_count: Option<&str>,
        interp: &str,
        extrapolation: &str,
        projection_grid: Option<Vec<f64>>,
        reset_lag: Option<i32>,
    ) -> PyResult<Self> {
        let base = py_to_date(base_date)?;
        let style = parse_interp_style(interp)?;
        let extrap = parse_extrapolation(extrapolation)?;

        let mut builder = ForwardCurve::builder(id, tenor)
            .base_date(base)
            .knots(knots)
            .interp(style)
            .extrapolation(extrap)
            .projection_grid_opt(projection_grid);
        if let Some(day_count) = day_count {
            builder = builder.day_count(parse_day_count(day_count)?);
        }
        if let Some(reset_lag) = reset_lag {
            builder = builder.reset_lag(reset_lag);
        }

        builder
            .build()
            .map(|curve| Self {
                inner: Arc::new(curve),
            })
            .map_err(core_to_py)
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
    ///     Day-count convention. When omitted, Rust infers a market default from the curve ID.
    /// interp : str, optional
    ///     Interpolation style (default ``"linear"``).
    /// extrapolation : str, optional
    ///     Extrapolation policy (default ``"flat_forward"``).
    /// projection_grid : list[float] | None, optional
    ///     Contractual reset/end-date projection boundaries. Omit for legacy
    ///     fixed numeric-tenor DF stepping.
    /// reset_lag : int | None, optional
    ///     Business days from fixing to spot. Omit for Rust curve-ID inference.
    #[new]
    #[expect(
        clippy::too_many_arguments,
        reason = "the cross-host constructor includes curve identity, data, and optional projection metadata"
    )]
    #[pyo3(signature = (id, tenor, knots, base_date, day_count=None, interp="linear", extrapolation="flat_forward", projection_grid=None, reset_lag=None))]
    fn new(
        id: &str,
        tenor: f64,
        knots: Vec<(f64, f64)>,
        base_date: &Bound<'_, PyAny>,
        day_count: Option<&str>,
        interp: &str,
        extrapolation: &str,
        projection_grid: Option<Vec<f64>>,
        reset_lag: Option<i32>,
    ) -> PyResult<Self> {
        Self::build(
            id,
            tenor,
            base_date,
            knots,
            day_count,
            interp,
            extrapolation,
            projection_grid,
            reset_lag,
        )
    }

    /// Construct from a keyword-only curve specification.
    #[classmethod]
    #[expect(
        clippy::too_many_arguments,
        reason = "the named factory exposes the complete curve specification"
    )]
    #[pyo3(signature = (id, *, tenor, base_date, knots, day_count=None, interp="linear", extrapolation="flat_forward", projection_grid=None, reset_lag=None))]
    fn from_knots(
        _cls: &Bound<'_, PyType>,
        id: &str,
        tenor: f64,
        base_date: &Bound<'_, PyAny>,
        knots: Vec<(f64, f64)>,
        day_count: Option<&str>,
        interp: &str,
        extrapolation: &str,
        projection_grid: Option<Vec<f64>>,
        reset_lag: Option<i32>,
    ) -> PyResult<Self> {
        Self::build(
            id,
            tenor,
            base_date,
            knots,
            day_count,
            interp,
            extrapolation,
            projection_grid,
            reset_lag,
        )
    }

    /// Forward rate at year fraction `t`.
    #[pyo3(text_signature = "(self, t)")]
    fn rate(&self, t: f64) -> f64 {
        self.inner.rate(t)
    }

    /// Discount-factor-implied simple forward rate over `(t1, t2)`.
    #[pyo3(text_signature = "(self, t1, t2)")]
    fn rate_between(&self, t1: f64, t2: f64) -> PyResult<f64> {
        self.inner.rate_between(t1, t2).map_err(core_to_py)
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

    /// Contractual projection boundaries, or `None` for legacy tenor stepping.
    #[getter]
    fn projection_grid(&self) -> Option<Vec<f64>> {
        self.inner.projection_grid().map(<[f64]>::to_vec)
    }

    /// Business days from fixing to spot.
    #[getter]
    fn reset_lag(&self) -> i32 {
        self.inner.reset_lag()
    }

    fn __repr__(&self) -> String {
        format!("ForwardCurve(id={:?})", self.inner.id().as_str())
    }
}

// ---------------------------------------------------------------------------
