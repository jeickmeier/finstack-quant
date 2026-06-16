//! Credit index data bindings.

use std::sync::Arc;

use finstack_quant_core::market_data::term_structures::{BaseCorrelationCurve, CreditIndexData};
use pyo3::prelude::*;

use super::hazard::PyHazardCurve;
use crate::errors::core_to_py;

// PyBaseCorrelationCurve
// ---------------------------------------------------------------------------

/// Base-correlation curve for synthetic credit index tranche pricing.
#[pyclass(
    name = "BaseCorrelationCurve",
    module = "finstack_quant.core.market_data.curves",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyBaseCorrelationCurve {
    /// Shared Rust curve.
    pub(crate) inner: Arc<BaseCorrelationCurve>,
}

#[pymethods]
impl PyBaseCorrelationCurve {
    /// Construct a base-correlation curve from `(detachment_pct, correlation)` knots.
    #[new]
    #[pyo3(signature = (id, knots))]
    fn new(id: &str, knots: Vec<(f64, f64)>) -> PyResult<Self> {
        let curve = BaseCorrelationCurve::builder(id)
            .knots(knots)
            .build()
            .map_err(core_to_py)?;
        Ok(Self {
            inner: Arc::new(curve),
        })
    }

    /// Interpolated base correlation at a detachment point.
    #[pyo3(text_signature = "(self, detachment_pct)")]
    fn correlation(&self, detachment_pct: f64) -> f64 {
        self.inner.correlation(detachment_pct)
    }

    /// Curve identifier string.
    #[getter]
    fn id(&self) -> &str {
        self.inner.id.as_str()
    }

    fn __repr__(&self) -> String {
        format!("BaseCorrelationCurve(id={:?})", self.inner.id.as_str())
    }
}

// ---------------------------------------------------------------------------
// PyCreditIndexData
// ---------------------------------------------------------------------------

/// Credit index data bundle for synthetic tranche pricing.
#[pyclass(
    name = "CreditIndexData",
    module = "finstack_quant.core.market_data.curves",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyCreditIndexData {
    /// Rust credit index bundle.
    pub(crate) inner: CreditIndexData,
}

#[pymethods]
impl PyCreditIndexData {
    /// Construct homogeneous credit index data from index hazard and base correlation curves.
    #[new]
    #[pyo3(signature = (num_constituents, recovery_rate, index_credit_curve, base_correlation_curve))]
    fn new(
        num_constituents: u16,
        recovery_rate: f64,
        index_credit_curve: &PyHazardCurve,
        base_correlation_curve: &PyBaseCorrelationCurve,
    ) -> PyResult<Self> {
        let data = CreditIndexData::builder()
            .num_constituents(num_constituents)
            .recovery_rate(recovery_rate)
            .index_credit_curve(Arc::clone(&index_credit_curve.inner))
            .base_correlation_curve(Arc::clone(&base_correlation_curve.inner))
            .build()
            .map_err(core_to_py)?;
        Ok(Self { inner: data })
    }

    /// Number of constituents in the credit index.
    #[getter]
    fn num_constituents(&self) -> u16 {
        self.inner.num_constituents
    }

    /// Index recovery rate.
    #[getter]
    fn recovery_rate(&self) -> f64 {
        self.inner.recovery_rate
    }

    fn __repr__(&self) -> String {
        format!(
            "CreditIndexData(num_constituents={}, recovery_rate={})",
            self.inner.num_constituents, self.inner.recovery_rate
        )
    }
}
