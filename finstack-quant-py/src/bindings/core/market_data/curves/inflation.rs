//! Inflation curve bindings.

use finstack_quant_core::market_data::term_structures::InflationCurve;

use std::sync::Arc;

use pyo3::prelude::*;

use super::helpers::{parse_day_count, parse_interp_style};
use crate::bindings::core::dates::utils::{date_to_py, py_to_date};
use crate::errors::core_to_py;

// PyInflationCurve
// ---------------------------------------------------------------------------

/// CPI inflation curve for inflation-linked pricing and breakeven analysis.
///
/// Wraps [`InflationCurve`] from `finstack-quant-core`.
#[pyclass(
    name = "InflationCurve",
    module = "finstack_quant.core.market_data.curves",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyInflationCurve {
    /// Shared Rust curve.
    pub(crate) inner: Arc<InflationCurve>,
}

impl PyInflationCurve {
    /// Build from an existing `Arc<InflationCurve>`.
    pub(crate) fn from_inner(inner: Arc<InflationCurve>) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyInflationCurve {
    /// Construct an inflation curve from CPI knot points.
    #[new]
    #[pyo3(signature = (id, base_date, base_cpi, knots, day_count="act_365f", indexation_lag_months=3, interp="log_linear"))]
    fn new(
        id: &str,
        base_date: &Bound<'_, PyAny>,
        base_cpi: f64,
        knots: Vec<(f64, f64)>,
        day_count: &str,
        indexation_lag_months: u32,
        interp: &str,
    ) -> PyResult<Self> {
        let base = py_to_date(base_date)?;
        let dc = parse_day_count(day_count)?;
        let style = parse_interp_style(interp)?;

        let curve = InflationCurve::builder(id)
            .base_date(base)
            .base_cpi(base_cpi)
            .day_count(dc)
            .indexation_lag_months(indexation_lag_months)
            .knots(knots)
            .interp(style)
            .build()
            .map_err(core_to_py)?;

        Ok(Self {
            inner: Arc::new(curve),
        })
    }

    /// CPI level at year fraction `t`, without indexation lag.
    #[pyo3(text_signature = "(self, t)")]
    fn cpi(&self, t: f64) -> f64 {
        self.inner.cpi(t)
    }

    /// CPI level at year fraction `t`, with configured indexation lag applied.
    #[pyo3(text_signature = "(self, t)")]
    fn cpi_with_lag(&self, t: f64) -> f64 {
        self.inner.cpi_with_lag(t)
    }

    /// Principal indexation ratio at year fraction `t`.
    ///
    /// Returns ``cpi_with_lag(t) / base_cpi`` -- the factor by which the
    /// notional of an inflation-linked security is uplifted at `t`. This is the
    /// curve-level view of the ``inflation_index_ratio`` reported per cashflow
    /// by the valuations layer. No deflation floor is applied.
    ///
    /// Raises ``ValueError`` if the curve's base CPI is not strictly positive.
    #[pyo3(text_signature = "(self, t)")]
    fn index_ratio(&self, t: f64) -> PyResult<f64> {
        self.inner.index_ratio(t).map_err(core_to_py)
    }

    /// Annualized inflation rate between `t1` and `t2` using CAGR.
    #[pyo3(text_signature = "(self, t1, t2)")]
    fn inflation_rate(&self, t1: f64, t2: f64) -> f64 {
        self.inner.inflation_rate(t1, t2)
    }

    /// Simple non-compounded inflation rate between `t1` and `t2`.
    #[pyo3(text_signature = "(self, t1, t2)")]
    fn inflation_rate_simple(&self, t1: f64, t2: f64) -> f64 {
        self.inner.inflation_rate_simple(t1, t2)
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

    /// Day-count convention used by this curve.
    #[getter]
    fn day_count(&self) -> String {
        self.inner.day_count().to_string()
    }

    /// Indexation lag in months.
    #[getter]
    fn indexation_lag_months(&self) -> u32 {
        self.inner.indexation_lag_months()
    }

    /// Base CPI level at `t = 0`.
    #[getter]
    fn base_cpi(&self) -> f64 {
        self.inner.base_cpi()
    }

    fn __repr__(&self) -> String {
        format!("InflationCurve(id={:?})", self.inner.id().as_str())
    }
}

// ---------------------------------------------------------------------------
