//! Hazard and base-correlation curve bindings.

use finstack_core::market_data::term_structures::HazardCurve;

use std::sync::Arc;

use pyo3::prelude::*;

use super::helpers::parse_day_count;
use crate::bindings::core::dates::utils::{date_to_py, py_to_date};
use crate::errors::core_to_py;

// PyHazardCurve
// ---------------------------------------------------------------------------

/// Credit hazard-rate curve for default probability modeling.
///
/// Wraps [`HazardCurve`] from `finstack-core`.
#[pyclass(
    name = "HazardCurve",
    module = "finstack.core.market_data.curves",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyHazardCurve {
    /// Shared Rust curve.
    pub(crate) inner: Arc<HazardCurve>,
}

impl PyHazardCurve {
    /// Build from an existing `Arc<HazardCurve>`.
    pub(crate) fn from_inner(inner: Arc<HazardCurve>) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyHazardCurve {
    /// Construct a hazard curve from knot points.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Unique curve identifier (e.g. ``"ACME-HZD"``).
    /// base_date : datetime.date
    ///     Valuation date.
    /// knots : list[tuple[float, float]]
    ///     ``(time_years, hazard_rate)`` pairs.
    /// recovery_rate : float, optional
    ///     Recovery rate. Defaults to the credit assumptions registry value.
    /// day_count : str, optional
    ///     Day-count convention (default ``"act_365f"``).
    /// par_spreads : list[tuple[float, float]], optional
    ///     Market par-spread quotes in basis points used for rebootstrap risks.
    #[new]
    #[pyo3(signature = (id, base_date, knots, recovery_rate=None, day_count="act_365f", par_spreads=None))]
    fn new(
        id: &str,
        base_date: &Bound<'_, PyAny>,
        knots: Vec<(f64, f64)>,
        recovery_rate: Option<f64>,
        day_count: &str,
        par_spreads: Option<Vec<(f64, f64)>>,
    ) -> PyResult<Self> {
        let base = py_to_date(base_date)?;
        let dc = parse_day_count(day_count)?;
        let recovery_rate = match recovery_rate {
            Some(r) => r,
            None => finstack_core::credit::registry::default_market_recovery_rate()
                .map_err(core_to_py)?,
        };

        let mut builder = HazardCurve::builder(id)
            .base_date(base)
            .recovery_rate(recovery_rate)
            .day_count(dc)
            .knots(knots);
        if let Some(points) = par_spreads {
            builder = builder.par_spreads(points);
        }
        let curve = builder.build().map_err(core_to_py)?;

        Ok(Self {
            inner: Arc::new(curve),
        })
    }

    /// Survival probability at year fraction `t`.
    #[pyo3(text_signature = "(self, t)")]
    fn survival(&self, t: f64) -> f64 {
        self.inner.sp(t)
    }

    /// Instantaneous hazard rate at year fraction `t`.
    #[pyo3(text_signature = "(self, t)")]
    fn hazard_rate(&self, t: f64) -> f64 {
        self.inner.hazard_rate(t)
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
        format!("HazardCurve(id={:?})", self.inner.id().as_str())
    }
}

// ---------------------------------------------------------------------------
