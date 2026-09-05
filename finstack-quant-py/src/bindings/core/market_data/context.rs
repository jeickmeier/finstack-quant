//! Python bindings for [`finstack_quant_core::market_data::context::MarketContext`].

use std::collections::BTreeMap;
use std::sync::Arc;

use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::types::CurveId;
use pyo3::exceptions::PyTypeError;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyModule};
use pyo3::IntoPyObjectExt;

use crate::bindings::core::currency::extract_currency;
use crate::bindings::core::money::{decimal_to_py, money_from_amount, PyMoney};
use crate::bindings::date_utils::py_to_date;
use crate::errors::core_to_py;

use super::curves::{
    PyBaseCorrelationCurve, PyCreditIndexData, PyDiscountCurve, PyForwardCurve,
    PyFxDeltaVolSurface, PyHazardCurve, PyInflationCurve, PyPriceCurve, PyVolCube, PyVolSurface,
};
use super::fx::PyFxMatrix;
use super::scalars::{extract_exact_f64, PyInflationIndex, PyScalarTimeSeries};

/// Unified market data container for curves, surfaces, scalars and FX.
///
/// Curves are stored behind shared handles, so getters are cheap and the
/// context is cheap to clone. Every ``insert_*`` method returns ``self`` for
/// fluent chaining. ``id in context`` and ``len(context)`` are supported.
///
/// Example
/// -------
/// >>> from finstack_quant.core.market_data import DiscountCurve, MarketContext
/// >>> ctx = MarketContext().insert(DiscountCurve.flat("USD-OIS", "2025-01-01", 0.05))
/// >>> "USD-OIS" in ctx, len(ctx)
/// (True, 1)
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

    fn total_items(&self) -> usize {
        let stats = self.inner.stats();
        stats.total_curves
            + stats.surface_count
            + stats.vol_cube_count
            + stats.price_count
            + stats.series_count
            + stats.inflation_index_count
            + stats.credit_index_count
            + stats.dividend_schedule_count
            + stats.fx_delta_vol_surface_count
    }
}

/// Render a list of identifiers Python-style: ``['A', 'B']``.
fn ids_repr(ids: &[&str]) -> String {
    let quoted: Vec<String> = ids.iter().map(|id| format!("'{id}'")).collect();
    format!("[{}]", quoted.join(", "))
}

#[pymethods]
impl PyMarketContext {
    /// Create an empty market context.
    ///
    /// Example
    /// -------
    /// >>> from finstack_quant.core.market_data import MarketContext
    /// >>> MarketContext().is_empty()
    /// True
    #[new]
    fn new() -> Self {
        Self {
            inner: MarketContext::new(),
        }
    }

    /// Insert a curve or surface (fluent, returns ``self``).
    ///
    /// Parameters
    /// ----------
    /// curve : DiscountCurve | ForwardCurve | HazardCurve | InflationCurve | PriceCurve | BaseCorrelationCurve | VolSurface | FxDeltaVolSurface | VolCube
    ///     Object stored under its own ``id``. A ``PriceCurve`` with
    ///     ``kind="vol_index"`` is stored as a vol-index curve.
    ///
    /// Returns
    /// -------
    /// MarketContext
    ///     ``self``.
    ///
    /// Raises
    /// ------
    /// TypeError
    ///     If ``curve`` is not one of the supported types.
    #[pyo3(text_signature = "(self, curve)")]
    fn insert<'py>(
        mut slf: PyRefMut<'py, Self>,
        curve: &Bound<'py, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        if let Ok(discount_curve) = curve.extract::<PyRef<'_, PyDiscountCurve>>() {
            slf.inner.insert_mut(Arc::clone(&discount_curve.inner));
            return Ok(slf);
        }
        if let Ok(fc) = curve.extract::<PyRef<'_, PyForwardCurve>>() {
            slf.inner.insert_mut(Arc::clone(&fc.inner));
            return Ok(slf);
        }
        if let Ok(hc) = curve.extract::<PyRef<'_, PyHazardCurve>>() {
            slf.inner.insert_mut(Arc::clone(&hc.inner));
            return Ok(slf);
        }
        if let Ok(ic) = curve.extract::<PyRef<'_, PyInflationCurve>>() {
            slf.inner.insert_mut(Arc::clone(&ic.inner));
            return Ok(slf);
        }
        if let Ok(pc) = curve.extract::<PyRef<'_, PyPriceCurve>>() {
            slf.inner.insert_mut(Arc::clone(&pc.inner));
            return Ok(slf);
        }
        if let Ok(bc) = curve.extract::<PyRef<'_, PyBaseCorrelationCurve>>() {
            slf.inner.insert_mut(Arc::clone(&bc.inner));
            return Ok(slf);
        }
        if let Ok(vs) = curve.extract::<PyRef<'_, PyVolSurface>>() {
            slf.inner.insert_surface_mut(Arc::clone(&vs.inner));
            return Ok(slf);
        }
        if let Ok(fxd) = curve.extract::<PyRef<'_, PyFxDeltaVolSurface>>() {
            slf.inner
                .insert_fx_delta_vol_surface_mut(Arc::clone(&fxd.inner));
            return Ok(slf);
        }
        if let Ok(vc) = curve.extract::<PyRef<'_, PyVolCube>>() {
            slf.inner.insert_vol_cube_mut(Arc::clone(&vc.inner));
            return Ok(slf);
        }
        Err(PyTypeError::new_err(
            "insert() expects a DiscountCurve, ForwardCurve, HazardCurve, InflationCurve, PriceCurve, BaseCorrelationCurve, VolSurface, FxDeltaVolSurface, or VolCube",
        ))
    }

    /// Attach an FX matrix (fluent, returns ``self``).
    ///
    /// Parameters
    /// ----------
    /// fx : FxMatrix
    ///     Matrix shared by reference; later quote updates are visible here.
    ///
    /// Returns
    /// -------
    /// MarketContext
    ///     ``self``.
    #[pyo3(text_signature = "(self, fx)")]
    fn insert_fx<'py>(mut slf: PyRefMut<'py, Self>, fx: &PyFxMatrix) -> PyRefMut<'py, Self> {
        slf.inner.insert_fx_mut(Arc::clone(&fx.inner));
        slf
    }

    /// Insert a scalar market price (fluent, returns ``self``).
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Identifier for the scalar.
    /// value : float | int | decimal.Decimal
    ///     Price or unitless value. Monetary ``Decimal`` values keep full
    ///     precision; unitless ``Decimal`` values must round-trip through ``float``.
    /// currency : Currency | str, optional
    ///     When given, the scalar is a monetary price in this currency;
    ///     otherwise it is unitless.
    ///
    /// Returns
    /// -------
    /// MarketContext
    ///     ``self``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` is non-finite or a unitless ``Decimal`` is not exactly representable.
    #[pyo3(signature = (id, value, currency=None), text_signature = "(self, id, value, currency=None)")]
    fn insert_price<'py>(
        mut slf: PyRefMut<'py, Self>,
        id: &str,
        value: &Bound<'_, PyAny>,
        currency: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let scalar = if let Some(raw_currency) = currency {
            let currency = extract_currency(raw_currency)?;
            MarketScalar::Price(money_from_amount(value, currency)?)
        } else {
            MarketScalar::Unitless(extract_exact_f64(value, "price value")?)
        };
        slf.inner.insert_price_mut(id, scalar);
        Ok(slf)
    }

    /// Insert credit index data under ``id`` (fluent, returns ``self``).
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Identifier for the index bundle (e.g. ``"CDX-IG"``); the bundle
    ///     carries no id of its own, so it must be supplied.
    /// data : CreditIndexData
    ///     Bundle to store.
    ///
    /// Returns
    /// -------
    /// MarketContext
    ///     ``self``.
    #[pyo3(text_signature = "(self, id, data)")]
    fn insert_credit_index<'py>(
        mut slf: PyRefMut<'py, Self>,
        id: &str,
        data: &PyCreditIndexData,
    ) -> PyRefMut<'py, Self> {
        // Cold path: one deep clone per insert; lookups now share the `Arc`.
        slf.inner.insert_credit_index_mut(id, (*data.inner).clone());
        slf
    }

    /// Insert a scalar time series under its own id (fluent, returns ``self``).
    ///
    /// Parameters
    /// ----------
    /// series : ScalarTimeSeries
    ///     Series to store.
    ///
    /// Returns
    /// -------
    /// MarketContext
    ///     ``self``.
    #[pyo3(text_signature = "(self, series)")]
    fn insert_series<'py>(
        mut slf: PyRefMut<'py, Self>,
        series: &PyScalarTimeSeries,
    ) -> PyRefMut<'py, Self> {
        slf.inner.insert_series_mut(series.inner.clone());
        slf
    }

    /// Insert an inflation index under its own id (fluent, returns ``self``).
    ///
    /// Parameters
    /// ----------
    /// index : InflationIndex
    ///     Index to store.
    ///
    /// Returns
    /// -------
    /// MarketContext
    ///     ``self``.
    #[pyo3(text_signature = "(self, index)")]
    fn insert_inflation_index<'py>(
        mut slf: PyRefMut<'py, Self>,
        index: &PyInflationIndex,
    ) -> PyRefMut<'py, Self> {
        let id = index.inner.id.clone();
        slf.inner
            .insert_inflation_index_mut(id, Arc::clone(&index.inner));
        slf
    }

    /// Map a CSA code to a discount curve id for collateral discounting (fluent).
    ///
    /// Parameters
    /// ----------
    /// csa_code : str
    ///     Collateral agreement identifier (e.g. ``"USD-CSA"``).
    /// discount_id : str
    ///     Id of a discount curve already inserted (or to be inserted) in this context.
    ///
    /// Returns
    /// -------
    /// MarketContext
    ///     ``self``.
    #[pyo3(text_signature = "(self, csa_code, discount_id)")]
    fn map_collateral<'py>(
        mut slf: PyRefMut<'py, Self>,
        csa_code: &str,
        discount_id: &str,
    ) -> PyRefMut<'py, Self> {
        slf.inner
            .map_collateral_mut(csa_code, CurveId::from(discount_id));
        slf
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
    ///
    /// Raises ``KeyError`` if the curve does not exist, ``ValueError`` if it is not a base-correlation curve.
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

    /// Retrieve a price curve (``kind="price"``) by identifier.
    ///
    /// Raises ``KeyError`` if the curve does not exist, ``ValueError`` if it is not a price curve.
    #[pyo3(text_signature = "(self, id)")]
    fn get_price_curve(&self, id: &str) -> PyResult<PyPriceCurve> {
        let arc = self.inner.get_price_curve(id).map_err(core_to_py)?;
        Ok(PyPriceCurve::from_inner(arc))
    }

    /// Retrieve a volatility-index curve (``PriceCurve`` with ``kind="vol_index"``) by identifier.
    ///
    /// Raises ``KeyError`` if the curve does not exist, ``ValueError`` if it is not a vol-index curve.
    #[pyo3(text_signature = "(self, id)")]
    fn get_vol_index_curve(&self, id: &str) -> PyResult<PyPriceCurve> {
        let arc = self.inner.get_vol_index_curve(id).map_err(core_to_py)?;
        Ok(PyPriceCurve::from_inner(arc))
    }

    /// Retrieve a scalar market price as ``(value, currency)``.
    ///
    /// Currency-tagged values return a lossless Python ``Decimal`` and ISO
    /// currency code. Unitless values return a ``float`` and ``None``.
    ///
    /// Raises ``KeyError`` if no scalar is stored under ``id``.
    #[pyo3(text_signature = "(self, id)")]
    fn get_price(&self, py: Python<'_>, id: &str) -> PyResult<(Py<PyAny>, Option<String>)> {
        match self.inner.get_price(id).map_err(core_to_py)? {
            MarketScalar::Unitless(value) => Ok((value.into_py_any(py)?, None)),
            MarketScalar::Price(money) => Ok((
                decimal_to_py(py, money.amount_decimal())?.unbind(),
                Some(money.currency().to_string()),
            )),
        }
    }

    /// Retrieve a scalar time series by identifier.
    ///
    /// Raises ``KeyError`` if no series is stored under ``id``.
    #[pyo3(text_signature = "(self, id)")]
    fn get_series(&self, id: &str) -> PyResult<PyScalarTimeSeries> {
        self.inner
            .get_series(id)
            .cloned()
            .map(PyScalarTimeSeries::from_inner)
            .map_err(core_to_py)
    }

    /// Retrieve an inflation index by identifier.
    ///
    /// Raises ``KeyError`` if no index is stored under ``id``.
    #[pyo3(text_signature = "(self, id)")]
    fn get_inflation_index(&self, id: &str) -> PyResult<PyInflationIndex> {
        let arc = self.inner.get_inflation_index(id).map_err(core_to_py)?;
        Ok(PyInflationIndex::from_inner(arc))
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
    /// Raises ``KeyError`` if the surface does not exist.
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

    /// Retrieve credit-index data by identifier.
    ///
    /// Raises ``KeyError`` if no bundle is stored under ``id``.
    #[pyo3(text_signature = "(self, id)")]
    fn get_credit_index(&self, id: &str) -> PyResult<PyCreditIndexData> {
        let arc = self.inner.get_credit_index(id).map_err(core_to_py)?;
        Ok(PyCreditIndexData::from_inner(arc))
    }

    /// Access the FX matrix (``None`` if not set).
    #[getter]
    fn fx(&self) -> Option<PyFxMatrix> {
        self.inner.fx().map(|arc_fx| PyFxMatrix {
            inner: Arc::clone(arc_fx),
        })
    }

    /// Access the FX matrix, raising if none is attached.
    ///
    /// Returns
    /// -------
    /// FxMatrix
    ///
    /// Raises
    /// ------
    /// KeyError
    ///     If no FX matrix has been inserted.
    #[pyo3(text_signature = "(self)")]
    fn fx_required(&self) -> PyResult<PyFxMatrix> {
        self.inner
            .fx_required()
            .map(|fx| PyFxMatrix {
                inner: Arc::clone(fx),
            })
            .map_err(core_to_py)
    }

    /// Convert a monetary amount into another currency with the attached FX matrix.
    ///
    /// Same-currency amounts are returned unchanged without consulting FX.
    ///
    /// Parameters
    /// ----------
    /// amount : Money
    ///     Amount to convert.
    /// target_currency : Currency | str
    ///     Destination currency.
    /// as_of : datetime.date | str
    ///     Date used for the FX rate lookup.
    ///
    /// Returns
    /// -------
    /// Money
    ///
    /// Raises
    /// ------
    /// KeyError
    ///     If no FX matrix is attached or the pair cannot be resolved.
    #[pyo3(text_signature = "(self, amount, target_currency, as_of)")]
    fn convert_money(
        &self,
        amount: &PyMoney,
        target_currency: &Bound<'_, PyAny>,
        as_of: &Bound<'_, PyAny>,
    ) -> PyResult<PyMoney> {
        self.inner
            .convert_money(
                amount.inner,
                extract_currency(target_currency)?,
                py_to_date(as_of)?,
            )
            .map(PyMoney::from_inner)
            .map_err(core_to_py)
    }

    /// Whether any object (curve, surface, scalar, series, index, credit index
    /// or collateral mapping) is stored under ``id``.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Identifier to look up.
    ///
    /// Returns
    /// -------
    /// bool
    #[pyo3(text_signature = "(self, id)")]
    fn contains(&self, id: &str) -> bool {
        self.inner.contains(id)
    }

    fn __contains__(&self, id: &str) -> bool {
        self.inner.contains(id)
    }

    /// Identifiers of all stored term-structure curves, sorted.
    ///
    /// Returns
    /// -------
    /// list[str]
    #[pyo3(text_signature = "(self)")]
    fn curve_ids(&self) -> Vec<String> {
        let mut ids: Vec<String> = self
            .inner
            .curve_ids()
            .map(|id| id.as_str().to_owned())
            .collect();
        ids.sort();
        ids
    }

    /// Whether nothing has been inserted (no curves, surfaces, scalars or FX).
    ///
    /// Returns
    /// -------
    /// bool
    #[pyo3(text_signature = "(self)")]
    fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Counts of stored objects by category.
    ///
    /// Returns
    /// -------
    /// dict
    ///     Keys: ``curve_counts`` (dict of curve type to count), ``total_curves``,
    ///     ``has_fx``, ``surface_count``, ``vol_cube_count``, ``price_count``,
    ///     ``series_count``, ``inflation_index_count``, ``credit_index_count``,
    ///     ``dividend_schedule_count``, ``fx_delta_vol_surface_count``,
    ///     ``collateral_mapping_count``.
    #[pyo3(text_signature = "(self)")]
    fn stats<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let stats = self.inner.stats();
        let out = PyDict::new(py);
        let counts = PyDict::new(py);
        for (kind, count) in &stats.curve_counts {
            counts.set_item(*kind, *count)?;
        }
        out.set_item("curve_counts", counts)?;
        out.set_item("total_curves", stats.total_curves)?;
        out.set_item("has_fx", stats.has_fx)?;
        out.set_item("surface_count", stats.surface_count)?;
        out.set_item("vol_cube_count", stats.vol_cube_count)?;
        out.set_item("price_count", stats.price_count)?;
        out.set_item("series_count", stats.series_count)?;
        out.set_item("inflation_index_count", stats.inflation_index_count)?;
        out.set_item("credit_index_count", stats.credit_index_count)?;
        out.set_item("dividend_schedule_count", stats.dividend_schedule_count)?;
        out.set_item(
            "fx_delta_vol_surface_count",
            stats.fx_delta_vol_surface_count,
        )?;
        out.set_item("collateral_mapping_count", stats.collateral_mapping_count)?;
        Ok(out)
    }

    /// Roll every dated term structure forward by ``days`` calendar days.
    ///
    /// Curves keep their shape in time-from-base terms; base dates advance.
    ///
    /// Parameters
    /// ----------
    /// days : int
    ///     Calendar days to roll (may be negative).
    ///
    /// Returns
    /// -------
    /// MarketContext
    ///     New context; ``self`` is unchanged.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a curve cannot be rebuilt after rolling.
    #[pyo3(text_signature = "(self, days)")]
    fn roll_forward(&self, days: i64) -> PyResult<Self> {
        self.inner
            .roll_forward(days)
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    /// Number of stored objects (curves, surfaces, cubes, scalars, series, indices, credit indices).
    fn __len__(&self) -> usize {
        self.total_items()
    }

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    ///
    /// Reconstruction goes through the same strict serde round-trip as
    /// `to_json` / `from_json`, so an unpickled value is exactly what the wire
    /// format defines — there is no second state format that can drift.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Deserialize a market context from a JSON string.
    ///
    /// Accepts the same JSON format produced by :meth:`to_json` and by the
    /// calibration and pricing pipelines.
    ///
    /// Raises ``ValueError`` if the JSON is malformed or fails validation.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let ctx: MarketContext = serde_json::from_str(json)
            .map_err(|e| crate::errors::value_error(format!("invalid MarketContext JSON: {e}")))?;
        Ok(Self { inner: ctx })
    }

    /// Serialize this market context to compact JSON (round-trips with pricers).
    ///
    /// Raises ``ValueError`` if serialization fails.
    #[pyo3(text_signature = "(self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(|e| {
            crate::errors::value_error(format!("failed to serialize MarketContext: {e}"))
        })
    }

    fn __repr__(&self) -> String {
        let mut by_type: BTreeMap<&'static str, Vec<&str>> = BTreeMap::new();
        for (id, storage) in self.inner.iter_curves() {
            by_type
                .entry(storage.curve_type())
                .or_default()
                .push(id.as_str());
        }
        let mut parts: Vec<String> = Vec::new();
        for (kind, ids) in by_type.iter_mut() {
            ids.sort_unstable();
            parts.push(format!("{}={}", kind.to_lowercase(), ids_repr(ids)));
        }
        let mut push_ids = |label: &str, mut ids: Vec<&str>| {
            if !ids.is_empty() {
                ids.sort_unstable();
                parts.push(format!("{label}={}", ids_repr(&ids)));
            }
        };
        let surfaces = self.inner.surfaces_snapshot();
        push_ids("surfaces", surfaces.keys().map(|id| id.as_str()).collect());
        push_ids(
            "vol_cubes",
            self.inner
                .vol_cubes_iter()
                .map(|(id, _)| id.as_str())
                .collect(),
        );
        push_ids(
            "fx_delta_vol_surfaces",
            self.inner
                .fx_delta_vol_surfaces_iter()
                .map(|(id, _)| id.as_str())
                .collect(),
        );
        push_ids(
            "prices",
            self.inner
                .prices_iter()
                .map(|(id, _)| id.as_str())
                .collect(),
        );
        push_ids(
            "series",
            self.inner
                .series_iter()
                .map(|(id, _)| id.as_str())
                .collect(),
        );
        push_ids(
            "inflation_indices",
            self.inner
                .inflation_indices_iter()
                .map(|(id, _)| id.as_str())
                .collect(),
        );
        let stats = self.inner.stats();
        if stats.credit_index_count > 0 {
            parts.push(format!("credit_indices={}", stats.credit_index_count));
        }
        parts.push(format!(
            "fx={}",
            if stats.has_fx { "True" } else { "False" }
        ));
        format!("MarketContext({})", parts.join(", "))
    }
}

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
