//! Inflation, cross-currency, CDS tranche, FX spot, and price quote Python bindings.

use super::super::envelope::from_value;
use super::rates_vol::{PyCdsQuote, PyRateQuote, PyVolQuote, extract_pillar};
use crate::bindings::core::currency::extract_currency;
use crate::bindings::core::market_data::scalars::extract_f64;
use crate::bindings::core::money::{decimal_to_py, money_from_amount};
use crate::bindings::date_utils::{date_to_py, py_to_date};
use crate::bindings::extract::extract_basis_points;
use crate::bindings::json_bridge::serde_to_py;
use crate::bindings::macros::wire_methods;
use crate::bindings::module_utils::{py_to_json_value, py_to_serde};
use crate::bindings::pydoc::pydoc;
use crate::bindings::repr_support::repr_from_serde;
use crate::errors::core_to_py;
use finstack_quant_calibration::api::market_datum::{FxSpotDatum, MarketDatum, PriceDatum};
use finstack_quant_calibration::quotes::CdsTrancheQuote;
use finstack_quant_calibration::quotes::InflationQuote;
use finstack_quant_calibration::quotes::XccyQuote;
use finstack_quant_core::market_data::scalars::MarketScalar;
use pyo3::IntoPyObjectExt;
use pyo3::prelude::*;

/// Inflation-swap quote (zero-coupon or year-on-year) for inflation-curve calibration.
///
/// Examples
/// --------
/// >>> from finstack_quant.calibration import InflationQuote
/// >>> q = InflationQuote.inflation_swap("USCPI-ZC-5Y", "2031-05-08", 0.025, "USA-CPI-U", "USD")
/// >>> q.id, q.type, q.rate
/// ('USCPI-ZC-5Y', 'inflation_swap', 0.025)
#[pyclass(
    name = "InflationQuote",
    module = "finstack_quant.calibration",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyInflationQuote {
    pub(crate) inner: InflationQuote,
}

impl PyInflationQuote {
    pub(crate) fn from_inner(inner: InflationQuote) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyInflationQuote {
    /// Zero-coupon inflation swap quote.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Unique quote identifier.
    /// maturity : datetime.date | str
    ///     Swap maturity date (ISO string or ``date``).
    /// rate : float
    ///     Fixed zero-coupon swap rate as a decimal (``0.025`` = 2.5%).
    /// index : str
    ///     Inflation index identifier (e.g. ``"USA-CPI-U"``).
    /// convention : str
    ///     Inflation-swap convention identifier (e.g. ``"USD"``).
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the date cannot be parsed or the rate is not finite.
    #[staticmethod]
    #[pyo3(text_signature = "(id, maturity, rate, index, convention)")]
    fn inflation_swap(
        id: &str,
        maturity: &Bound<'_, PyAny>,
        rate: f64,
        index: &str,
        convention: &str,
    ) -> PyResult<Self> {
        InflationQuote::inflation_swap(id, py_to_date(maturity)?, rate, index, convention)
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    /// Year-on-year inflation swap quote.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Unique quote identifier.
    /// maturity : datetime.date | str
    ///     Swap maturity date (ISO string or ``date``).
    /// rate : float
    ///     Fixed year-on-year swap rate as a decimal.
    /// index : str
    ///     Inflation index identifier (e.g. ``"USA-CPI-U"``).
    /// frequency : str
    ///     Payment frequency tenor (e.g. ``"1Y"``).
    /// convention : str
    ///     Inflation-swap convention identifier (e.g. ``"USD"``).
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the date or frequency cannot be parsed or the rate is not finite.
    #[staticmethod]
    #[pyo3(text_signature = "(id, maturity, rate, index, frequency, convention)")]
    fn yoy_inflation_swap(
        id: &str,
        maturity: &Bound<'_, PyAny>,
        rate: f64,
        index: &str,
        frequency: &str,
        convention: &str,
    ) -> PyResult<Self> {
        InflationQuote::yoy_inflation_swap(
            id,
            py_to_date(maturity)?,
            rate,
            index,
            frequency,
            convention,
        )
        .map(Self::from_inner)
        .map_err(core_to_py)
    }

    /// Unique quote identifier.
    #[getter]
    fn id(&self) -> String {
        self.inner.id().as_str().to_string()
    }

    /// Quote type: ``"inflation_swap"`` or ``"yoy_inflation_swap"``.
    #[getter]
    #[pyo3(name = "type")]
    fn quote_type(&self) -> &'static str {
        match self.inner {
            InflationQuote::InflationSwap { .. } => "inflation_swap",
            InflationQuote::YoYInflationSwap { .. } => "yoy_inflation_swap",
        }
    }

    /// Swap maturity date.
    #[getter]
    fn maturity<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.maturity())
    }

    /// Fixed swap rate as a decimal.
    #[getter]
    fn rate(&self) -> f64 {
        self.inner.rate()
    }

    /// Inflation index identifier.
    #[getter]
    fn index(&self) -> String {
        self.inner.index().to_string()
    }

    /// Payment frequency tenor of a year-on-year quote; ``None`` for zero-coupon.
    #[getter]
    fn frequency(&self) -> Option<String> {
        match &self.inner {
            InflationQuote::InflationSwap { .. } => None,
            InflationQuote::YoYInflationSwap { frequency, .. } => Some(frequency.to_string()),
        }
    }

    /// Inflation-swap convention identifier.
    #[getter]
    fn convention(&self) -> String {
        self.inner.convention().to_string()
    }

    fn __repr__(&self) -> String {
        repr_from_serde("InflationQuote", &self.inner)
    }
}

wire_methods!(PyInflationQuote, InflationQuote, "InflationQuote", validate);

/// Cross-currency basis-swap quote for ``xccy_basis`` calibration steps.
///
/// Examples
/// --------
/// >>> from finstack_quant.calibration import XccyQuote
/// >>> q = XccyQuote("EURUSD-XCCY-5Y", "EUR-USD", "5Y", -12.5)
/// >>> q.id, q.basis_spread_bp, q.spot_fx is None
/// ('EURUSD-XCCY-5Y', -12.5, True)
#[pyclass(
    name = "XccyQuote",
    module = "finstack_quant.calibration",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyXccyQuote {
    pub(crate) inner: XccyQuote,
}

#[pymethods]
impl PyXccyQuote {
    /// Build a validated cross-currency basis-swap quote.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Unique quote identifier.
    /// convention : str
    ///     Cross-currency swap convention identifier (e.g. ``"EUR-USD"``).
    /// far_pillar : str | datetime.date | dict
    ///     Swap maturity pillar (tenor string such as ``"5Y"``, date, or mapping).
    /// basis_spread_bp : float | Bps
    ///     Basis spread in basis points.
    /// spot_fx : float | None, default None
    ///     Spot FX rate override (quote currency per base currency, > 0);
    ///     ``None`` uses the market FX.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the pillar cannot be parsed, the spread is not finite, or
    ///     ``spot_fx`` is not positive.
    #[new]
    #[pyo3(signature = (id, convention, far_pillar, basis_spread_bp, spot_fx = None))]
    #[pyo3(text_signature = "(id, convention, far_pillar, basis_spread_bp, spot_fx=None)")]
    fn new(
        py: Python<'_>,
        id: &str,
        convention: &str,
        far_pillar: &Bound<'_, PyAny>,
        basis_spread_bp: &Bound<'_, PyAny>,
        spot_fx: Option<f64>,
    ) -> PyResult<Self> {
        XccyQuote::new(
            id,
            convention,
            extract_pillar(py, far_pillar)?,
            extract_basis_points(basis_spread_bp, "basis_spread_bp")?,
            spot_fx,
        )
        .map(|inner| Self { inner })
        .map_err(core_to_py)
    }

    /// Unique quote identifier.
    #[getter]
    fn id(&self) -> String {
        self.inner.id.as_str().to_string()
    }

    /// Cross-currency swap convention identifier.
    #[getter]
    fn convention(&self) -> String {
        self.inner.convention.to_string()
    }

    /// Maturity pillar as its wire mapping (``{"tenor": ...}`` or ``{"date": ...}``).
    #[getter]
    fn far_pillar<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.far_pillar)
    }

    /// Basis spread in basis points.
    #[getter]
    fn basis_spread_bp(&self) -> f64 {
        self.inner.basis_spread_bp
    }

    /// Spot FX rate override, when set.
    #[getter]
    fn spot_fx(&self) -> Option<f64> {
        self.inner.spot_fx
    }

    fn __repr__(&self) -> String {
        repr_from_serde("XccyQuote", &self.inner)
    }
}

wire_methods!(PyXccyQuote, XccyQuote, "XccyQuote", validate);

/// CDS index tranche quote for ``base_correlation`` calibration steps.
///
/// Examples
/// --------
/// >>> from finstack_quant.calibration import CdsTrancheQuote
/// >>> q = CdsTrancheQuote({
/// ...     "id": "IG-3-7", "index": "CDX.NA.IG", "series": 42,
/// ...     "attachment": 0.03, "detachment": 0.07, "maturity": "2031-06-20",
/// ...     "upfront_pct": 0.01, "coupon_bp": 100.0,
/// ...     "convention": {"currency": "USD", "doc_clause": "isda_na"},
/// ... })
/// >>> q.id, q.attachment, q.detachment
/// ('IG-3-7', 0.03, 0.07)
#[pyclass(
    name = "CdsTrancheQuote",
    module = "finstack_quant.calibration",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyCdsTrancheQuote {
    pub(crate) inner: CdsTrancheQuote,
}

#[pymethods]
impl PyCdsTrancheQuote {
    #[doc = pydoc!("finstack_quant.calibration._quotes.market", "CdsTrancheQuote.__init__")]
    #[new]
    #[pyo3(text_signature = "(params)")]
    fn new(params: &Bound<'_, PyAny>) -> PyResult<Self> {
        let inner: CdsTrancheQuote = py_to_serde(params.py(), params, "CdsTrancheQuote params")?;
        inner.validate().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    /// Unique quote identifier.
    #[getter]
    fn id(&self) -> String {
        self.inner.id.as_str().to_string()
    }

    /// Credit index name.
    #[getter]
    fn index(&self) -> String {
        self.inner.index.clone()
    }

    /// Index series number.
    #[getter]
    fn series(&self) -> u16 {
        self.inner.series
    }

    /// Attachment point as a decimal loss fraction.
    #[getter]
    fn attachment(&self) -> f64 {
        self.inner.attachment
    }

    /// Detachment point as a decimal loss fraction.
    #[getter]
    fn detachment(&self) -> f64 {
        self.inner.detachment
    }

    /// Tranche maturity date.
    #[getter]
    fn maturity<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.maturity)
    }

    /// Upfront payment as a decimal fraction of notional.
    #[getter]
    fn upfront_pct(&self) -> f64 {
        self.inner.upfront_pct
    }

    /// Running coupon in basis points.
    #[getter]
    fn coupon_bp(&self) -> f64 {
        self.inner.coupon_bp
    }

    /// CDS convention as ``{"currency": ..., "doc_clause": ...}``.
    #[getter]
    fn convention<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.convention)
    }

    fn __repr__(&self) -> String {
        repr_from_serde("CdsTrancheQuote", &self.inner)
    }
}

wire_methods!(
    PyCdsTrancheQuote,
    CdsTrancheQuote,
    "CdsTrancheQuote",
    validate
);

/// FX spot rate market datum (``kind == "fx_spot"``).
///
/// ``from_`` carries the Rust ``from`` field: ``from`` is a Python keyword.
///
/// Examples
/// --------
/// >>> from finstack_quant.calibration import FxSpotDatum
/// >>> d = FxSpotDatum("EURUSD", "EUR", "USD", 1.1)
/// >>> d.from_, d.to, d.rate
/// ('EUR', 'USD', 1.1)
#[pyclass(
    name = "FxSpotDatum",
    module = "finstack_quant.calibration",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyFxSpotDatum {
    pub(crate) inner: FxSpotDatum,
}

#[pymethods]
impl PyFxSpotDatum {
    /// Build an FX spot datum.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Stable datum identifier (e.g. ``"EURUSD"``).
    /// from_ : str | Currency
    ///     Base currency (``EUR`` in ``EUR/USD``); the Rust ``from`` field.
    /// to : str | Currency
    ///     Quote currency (``USD`` in ``EUR/USD``).
    /// rate : float
    ///     Units of ``to`` per one unit of ``from_``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a currency code is unknown.
    #[new]
    #[pyo3(text_signature = "(id, from_, to, rate)")]
    fn new(id: &str, from_: &Bound<'_, PyAny>, to: &Bound<'_, PyAny>, rate: f64) -> PyResult<Self> {
        Ok(Self {
            inner: FxSpotDatum {
                id: id.to_string(),
                from: extract_currency(from_)?,
                to: extract_currency(to)?,
                rate,
            },
        })
    }

    /// Stable datum identifier.
    #[getter]
    fn id(&self) -> String {
        self.inner.id.clone()
    }

    /// Base currency code (the Rust ``from`` field).
    #[getter]
    fn from_(&self) -> String {
        self.inner.from.to_string()
    }

    /// Quote currency code.
    #[getter]
    fn to(&self) -> String {
        self.inner.to.to_string()
    }

    /// Units of ``to`` per one unit of ``from_``.
    #[getter]
    fn rate(&self) -> f64 {
        self.inner.rate
    }

    fn __repr__(&self) -> String {
        repr_from_serde("FxSpotDatum", &self.inner)
    }
}

wire_methods!(PyFxSpotDatum, FxSpotDatum, "FxSpotDatum");

/// Single-asset spot price market datum (``kind == "price"``).
///
/// Examples
/// --------
/// >>> from finstack_quant.calibration import PriceDatum
/// >>> d = PriceDatum("AAPL", 187.5)
/// >>> d.id, d.value, d.currency
/// ('AAPL', 187.5, None)
#[pyclass(
    name = "PriceDatum",
    module = "finstack_quant.calibration",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyPriceDatum {
    pub(crate) inner: PriceDatum,
}

#[pymethods]
impl PyPriceDatum {
    /// Build a spot price datum.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Stable datum identifier (e.g. the asset ticker).
    /// value : float | int | decimal.Decimal
    ///     Spot value. Without ``currency`` it is a unitless scalar; with one
    ///     it is a monetary price.
    /// currency : str | Currency | None, default None
    ///     Currency of a monetary price; ``None`` stores a unitless scalar.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` is non-finite or the currency is unknown. A unitless
    ///     ``Decimal`` converts to ``float``.
    #[new]
    #[pyo3(signature = (id, value, currency = None))]
    #[pyo3(text_signature = "(id, value, currency=None)")]
    fn new(
        id: &str,
        value: &Bound<'_, PyAny>,
        currency: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        let scalar = match currency {
            Some(currency) => {
                MarketScalar::price(money_from_amount(value, extract_currency(currency)?)?)
            }
            None => MarketScalar::unitless(extract_f64(value, "value")?).map_err(core_to_py)?,
        };
        Ok(Self {
            inner: PriceDatum {
                id: id.to_string(),
                scalar,
            },
        })
    }

    /// Stable datum identifier.
    #[getter]
    fn id(&self) -> String {
        self.inner.id.clone()
    }

    /// Spot value: ``float`` when unitless, lossless ``Decimal`` for a monetary price.
    #[getter]
    fn value(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        if let Some(money) = self.inner.scalar.get_price() {
            Ok(decimal_to_py(py, money.amount_decimal())?.unbind())
        } else if let Some(value) = self.inner.scalar.get_unitless() {
            value.into_py_any(py)
        } else {
            Err(crate::errors::value_error(
                "PriceDatum scalar must be unitless or monetary",
            ))
        }
    }

    /// Currency code of a monetary price; ``None`` when unitless.
    #[getter]
    fn currency(&self) -> Option<String> {
        self.inner
            .scalar
            .get_price()
            .map(|money| money.currency().to_string())
    }

    fn __repr__(&self) -> String {
        repr_from_serde("PriceDatum", &self.inner)
    }
}

wire_methods!(PyPriceDatum, PriceDatum, "PriceDatum");

/// Convert one market-data entry (typed quote or datum, or dict) into a `MarketDatum`.
pub(crate) fn extract_market_datum(
    py: Python<'_>,
    obj: &Bound<'_, PyAny>,
) -> PyResult<MarketDatum> {
    if let Ok(quote) = obj.cast::<PyRateQuote>() {
        return Ok(MarketDatum::RateQuote(quote.borrow().inner.clone()));
    }
    if let Ok(quote) = obj.cast::<PyCdsQuote>() {
        return Ok(MarketDatum::CdsQuote(quote.borrow().inner.clone()));
    }
    if let Ok(quote) = obj.cast::<PyVolQuote>() {
        return Ok(MarketDatum::VolQuote(quote.borrow().inner.clone()));
    }
    if let Ok(quote) = obj.cast::<PyInflationQuote>() {
        return Ok(MarketDatum::InflationQuote(quote.borrow().inner.clone()));
    }
    if let Ok(quote) = obj.cast::<PyXccyQuote>() {
        return Ok(MarketDatum::XccyQuote(quote.borrow().inner.clone()));
    }
    if let Ok(quote) = obj.cast::<PyCdsTrancheQuote>() {
        return Ok(MarketDatum::CdsTrancheQuote(quote.borrow().inner.clone()));
    }
    if let Ok(datum) = obj.cast::<PyFxSpotDatum>() {
        return Ok(MarketDatum::FxSpot(datum.borrow().inner.clone()));
    }
    if let Ok(datum) = obj.cast::<PyPriceDatum>() {
        return Ok(MarketDatum::Price(datum.borrow().inner.clone()));
    }
    let value = py_to_json_value(py, obj, "market datum")?;
    from_value(value, "market datum (expected {\"kind\": ..., ...})")
}
