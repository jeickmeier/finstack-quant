//! Python bindings for [`finstack_quant_core::market_data::context::MarketContext`].

use std::sync::Arc;

use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::MarketScalar;
use pyo3::exceptions::PyTypeError;
use pyo3::prelude::*;
use pyo3::types::{PyList, PyModule};
use pyo3::IntoPyObjectExt;

use crate::bindings::core::currency::extract_currency;
use crate::bindings::core::money::{money_from_amount, PyMoney};
use crate::errors::core_to_py;

use super::curves::{
    PyBaseCorrelationCurve, PyCreditIndexData, PyDiscountCurve, PyForwardCurve,
    PyFxDeltaVolSurface, PyHazardCurve, PyInflationCurve, PyPriceCurve, PyVolCube, PyVolSurface,
    PyVolatilityIndexCurve,
};
use super::fx::PyFxMatrix;
use super::scalars::{extract_exact_f64, PyInflationIndex, PyScalarTimeSeries};

// ---------------------------------------------------------------------------
// PyMarketContext
// ---------------------------------------------------------------------------

/// Unified market data container for curves, surfaces, and FX.
///
/// Wraps [`MarketContext`] from `finstack-quant-core`. Curves are stored behind
/// `Arc` and the context is cheap to clone.
#[pyclass(
    name = "MarketContext",
    module = "finstack_quant.core.market_data.context",
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyMarketContext {
    /// Underlying Rust context.
    pub(crate) inner: MarketContext,
}

impl PyMarketContext {
    /// Construct from a Rust [`MarketContext`] (used by calibration and other bindings).
    pub(crate) fn from_inner(inner: MarketContext) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyMarketContext {
    /// Create an empty market context.
    #[new]
    fn new() -> Self {
        Self {
            inner: MarketContext::new(),
        }
    }

    /// Insert a curve into the context (fluent, returns ``self``).
    ///
    /// Accepts any curve type: ``DiscountCurve``, ``ForwardCurve``,
    /// ``HazardCurve``, ``InflationCurve``, ``PriceCurve``,
    /// ``BaseCorrelationCurve``, ``VolSurface``, ``FxDeltaVolSurface``,
    /// ``VolCube``, or ``VolatilityIndexCurve``.
    #[pyo3(text_signature = "(self, curve)")]
    fn insert<'py>(
        mut slf: PyRefMut<'py, Self>,
        curve: &Bound<'py, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        if let Ok(dc) = curve.extract::<PyRef<'_, PyDiscountCurve>>() {
            slf.inner = std::mem::take(&mut slf.inner).insert(Arc::clone(&dc.inner));
            return Ok(slf);
        }
        if let Ok(fc) = curve.extract::<PyRef<'_, PyForwardCurve>>() {
            slf.inner = std::mem::take(&mut slf.inner).insert(Arc::clone(&fc.inner));
            return Ok(slf);
        }
        if let Ok(hc) = curve.extract::<PyRef<'_, PyHazardCurve>>() {
            slf.inner = std::mem::take(&mut slf.inner).insert(Arc::clone(&hc.inner));
            return Ok(slf);
        }
        if let Ok(ic) = curve.extract::<PyRef<'_, PyInflationCurve>>() {
            slf.inner = std::mem::take(&mut slf.inner).insert(Arc::clone(&ic.inner));
            return Ok(slf);
        }
        if let Ok(pc) = curve.extract::<PyRef<'_, PyPriceCurve>>() {
            slf.inner = std::mem::take(&mut slf.inner).insert(Arc::clone(&pc.inner));
            return Ok(slf);
        }
        if let Ok(bc) = curve.extract::<PyRef<'_, PyBaseCorrelationCurve>>() {
            slf.inner = std::mem::take(&mut slf.inner).insert(Arc::clone(&bc.inner));
            return Ok(slf);
        }
        if let Ok(vs) = curve.extract::<PyRef<'_, PyVolSurface>>() {
            slf.inner = std::mem::take(&mut slf.inner).insert_surface(Arc::clone(&vs.inner));
            return Ok(slf);
        }
        if let Ok(fxd) = curve.extract::<PyRef<'_, PyFxDeltaVolSurface>>() {
            slf.inner =
                std::mem::take(&mut slf.inner).insert_fx_delta_vol_surface(Arc::clone(&fxd.inner));
            return Ok(slf);
        }
        if let Ok(vc) = curve.extract::<PyRef<'_, PyVolCube>>() {
            slf.inner = std::mem::take(&mut slf.inner).insert_vol_cube(Arc::clone(&vc.inner));
            return Ok(slf);
        }
        if let Ok(vc) = curve.extract::<PyRef<'_, PyVolatilityIndexCurve>>() {
            slf.inner = std::mem::take(&mut slf.inner).insert(Arc::clone(&vc.inner));
            return Ok(slf);
        }
        Err(PyTypeError::new_err(
            "insert() expects a DiscountCurve, ForwardCurve, HazardCurve, InflationCurve, PriceCurve, BaseCorrelationCurve, VolSurface, FxDeltaVolSurface, VolCube, or VolatilityIndexCurve",
        ))
    }

    /// Insert an FX matrix into the context.
    #[pyo3(text_signature = "(self, fx)")]
    fn insert_fx(&mut self, fx: &PyFxMatrix) {
        self.inner = std::mem::take(&mut self.inner).insert_fx(Arc::clone(&fx.inner));
    }

    /// Insert a scalar market price into the context.
    ///
    /// If ``currency`` is provided, the scalar is stored as a monetary price;
    /// otherwise it is stored as a unitless value. Monetary ``Decimal`` values
    /// preserve their full precision. Unitless ``Decimal`` values must
    /// round-trip through ``f64`` exactly. ``currency`` accepts a ``Currency``
    /// wrapper or an ISO code string.
    #[pyo3(signature = (id, value, currency=None), text_signature = "(self, id, value, currency=None)")]
    fn insert_price(
        &mut self,
        id: &str,
        value: &Bound<'_, PyAny>,
        currency: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        let scalar = if let Some(raw_currency) = currency {
            let currency = extract_currency(raw_currency)?;
            MarketScalar::Price(money_from_amount(value, currency)?)
        } else {
            MarketScalar::Unitless(extract_exact_f64(value, "price value")?)
        };
        self.inner = std::mem::take(&mut self.inner).insert_price(id, scalar);
        Ok(())
    }

    /// Insert credit index data into the context.
    #[pyo3(text_signature = "(self, id, data)")]
    fn insert_credit_index(&mut self, id: &str, data: &PyCreditIndexData) {
        self.inner = std::mem::take(&mut self.inner).insert_credit_index(id, data.inner.clone());
    }

    /// Insert a scalar time series into the context.
    #[pyo3(text_signature = "(self, series)")]
    fn insert_series(&mut self, series: &PyScalarTimeSeries) {
        self.inner = std::mem::take(&mut self.inner).insert_series(series.inner.clone());
    }

    /// Insert an inflation index into the context.
    #[pyo3(text_signature = "(self, index)")]
    fn insert_inflation_index(&mut self, index: &PyInflationIndex) {
        self.inner = std::mem::take(&mut self.inner)
            .insert_inflation_index(&index.inner.id, index.inner.clone());
    }

    /// Retrieve a discount curve by identifier.
    ///
    /// Raises ``KeyError`` if the curve does not exist, ``ValueError`` if it is not a discount curve.
    #[pyo3(text_signature = "(self, id)")]
    fn get_discount(&self, id: &str) -> PyResult<PyDiscountCurve> {
        let arc = self.inner.get_discount(id).map_err(core_to_py)?;
        Ok(PyDiscountCurve::from_inner(arc))
    }

    /// Retrieve a forward curve by identifier.
    ///
    /// Raises ``KeyError`` if the curve does not exist, ``ValueError`` if it is not a forward curve.
    #[pyo3(text_signature = "(self, id)")]
    fn get_forward(&self, id: &str) -> PyResult<PyForwardCurve> {
        let arc = self.inner.get_forward(id).map_err(core_to_py)?;
        Ok(PyForwardCurve::from_inner(arc))
    }

    /// Retrieve a hazard curve by identifier.
    ///
    /// Raises ``KeyError`` if the curve does not exist, ``ValueError`` if it is not a hazard curve.
    #[pyo3(text_signature = "(self, id)")]
    fn get_hazard(&self, id: &str) -> PyResult<PyHazardCurve> {
        let arc = self.inner.get_hazard(id).map_err(core_to_py)?;
        Ok(PyHazardCurve::from_inner(arc))
    }

    /// Retrieve a base-correlation curve by identifier.
    #[pyo3(text_signature = "(self, id)")]
    fn get_base_correlation(&self, id: &str) -> PyResult<PyBaseCorrelationCurve> {
        let arc = self.inner.get_base_correlation(id).map_err(core_to_py)?;
        Ok(PyBaseCorrelationCurve::from_inner(arc))
    }

    /// Retrieve an inflation curve by identifier.
    ///
    /// Raises ``KeyError`` if the curve does not exist, ``ValueError`` if it is not an inflation curve.
    #[pyo3(text_signature = "(self, id)")]
    fn get_inflation_curve(&self, id: &str) -> PyResult<PyInflationCurve> {
        let arc = self.inner.get_inflation_curve(id).map_err(core_to_py)?;
        Ok(PyInflationCurve::from_inner(arc))
    }

    /// Retrieve a price curve by identifier.
    ///
    /// Raises ``KeyError`` if the curve does not exist, ``ValueError`` if it is not a price curve.
    #[pyo3(text_signature = "(self, id)")]
    fn get_price_curve(&self, id: &str) -> PyResult<PyPriceCurve> {
        let arc = self.inner.get_price_curve(id).map_err(core_to_py)?;
        Ok(PyPriceCurve::from_inner(arc))
    }

    /// Retrieve a scalar market price by identifier.
    #[pyo3(text_signature = "(self, id)")]
    fn get_price(&self, py: Python<'_>, id: &str) -> PyResult<Py<PyAny>> {
        match self.inner.get_price(id).map_err(core_to_py)? {
            MarketScalar::Unitless(value) => value.into_py_any(py),
            MarketScalar::Price(money) => Ok(Py::new(py, PyMoney::from_inner(*money))?.into_any()),
        }
    }

    /// Retrieve a scalar time series by identifier.
    #[pyo3(text_signature = "(self, id)")]
    fn get_series(&self, id: &str) -> PyResult<PyScalarTimeSeries> {
        self.inner
            .get_series(id)
            .cloned()
            .map(PyScalarTimeSeries::from_inner)
            .map_err(core_to_py)
    }

    /// Retrieve an inflation index by identifier.
    #[pyo3(text_signature = "(self, id)")]
    fn get_inflation_index(&self, id: &str) -> PyResult<PyInflationIndex> {
        let arc = self.inner.get_inflation_index(id).map_err(core_to_py)?;
        Ok(PyInflationIndex::from_inner((*arc).clone()))
    }

    /// Retrieve a vol surface by identifier.
    ///
    /// Raises ``KeyError`` if the surface does not exist.
    #[pyo3(text_signature = "(self, id)")]
    fn get_surface(&self, id: &str) -> PyResult<PyVolSurface> {
        let arc = self.inner.get_surface(id).map_err(core_to_py)?;
        Ok(PyVolSurface::from_inner(arc))
    }

    /// Retrieve a delta-quoted FX vol surface by identifier.
    ///
    /// Raises ``KeyError`` if the surface does not exist, ``ValueError`` if it
    /// is not a delta-quoted FX surface.
    #[pyo3(text_signature = "(self, id)")]
    fn get_fx_delta_vol_surface(&self, id: &str) -> PyResult<PyFxDeltaVolSurface> {
        let arc = self
            .inner
            .get_fx_delta_vol_surface(id)
            .map_err(core_to_py)?;
        Ok(PyFxDeltaVolSurface::from_inner(arc))
    }

    /// Retrieve a vol cube by identifier.
    ///
    /// Raises ``KeyError`` if the cube does not exist.
    #[pyo3(text_signature = "(self, id)")]
    fn get_vol_cube(&self, id: &str) -> PyResult<PyVolCube> {
        let arc = self.inner.get_vol_cube(id).map_err(core_to_py)?;
        Ok(PyVolCube::from_inner(arc))
    }

    /// Retrieve a volatility index curve by identifier.
    ///
    /// Raises ``KeyError`` if the curve does not exist, ``ValueError`` if it is not a vol-index curve.
    #[pyo3(text_signature = "(self, id)")]
    fn get_vol_index_curve(&self, id: &str) -> PyResult<PyVolatilityIndexCurve> {
        let arc = self.inner.get_vol_index_curve(id).map_err(core_to_py)?;
        Ok(PyVolatilityIndexCurve::from_inner(arc))
    }

    /// Retrieve credit-index data by identifier.
    #[pyo3(text_signature = "(self, id)")]
    fn get_credit_index(&self, id: &str) -> PyResult<PyCreditIndexData> {
        let arc = self.inner.get_credit_index(id).map_err(core_to_py)?;
        Ok(PyCreditIndexData::from_inner((*arc).clone()))
    }

    /// Access the FX matrix (returns ``None`` if not set).
    #[getter]
    fn fx(&self) -> Option<PyFxMatrix> {
        self.inner.fx().map(|arc_fx| PyFxMatrix {
            inner: Arc::clone(arc_fx),
        })
    }

    /// Deserialize a market context from a JSON string.
    ///
    /// Accepts the same JSON format produced by :meth:`to_json` and by the
    /// calibration and pricing pipelines.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let ctx: MarketContext = serde_json::from_str(json)
            .map_err(|e| crate::errors::value_error(format!("invalid MarketContext JSON: {e}")))?;
        Ok(Self { inner: ctx })
    }

    /// Serialize this market context to pretty-printed JSON (round-trips with pricers).
    #[pyo3(text_signature = "(self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string_pretty(&self.inner).map_err(|e| {
            crate::errors::value_error(format!("failed to serialize MarketContext: {e}"))
        })
    }

    fn __repr__(&self) -> String {
        "MarketContext(...)".to_string()
    }
}

// ---------------------------------------------------------------------------
// Module registration
// ---------------------------------------------------------------------------

pub(super) const EXPORTS: &[&str] = &["MarketContext"];

/// Register the `finstack_quant.core.market_data.context` submodule.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(py, "context")?;
    m.setattr(
        "__doc__",
        "Market data context container bindings (finstack-quant-core).",
    )?;

    m.add_class::<PyMarketContext>()?;

    let all = PyList::new(py, EXPORTS)?;
    m.setattr("__all__", all)?;

    crate::bindings::module_utils::register_submodule(
        py,
        parent,
        &m,
        "context",
        "finstack_quant.core.market_data",
        crate::bindings::module_utils::ParentNameSource::Package,
    )?;

    Ok(())
}
