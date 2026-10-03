//! Typed market data beyond rate, CDS and vol quotes: inflation, cross-currency
//! and CDS-tranche quotes plus the FX-spot, price, dividend-schedule and
//! collateral datums carried by the calibration envelope.
//!
//! Quote constructors marshal their arguments into serde wire fields and call
//! the Rust `from_wire_fields` constructors, which own strict deserialization
//! and quote validation. The datum constructors only convert host values into
//! the Rust payload structs, which carry no validation of their own.

use super::envelope::{currency_code, extract_pillar, from_value};
use crate::bindings::core::currency::extract_currency;
use crate::bindings::core::market_data::scalars::extract_exact_f64;
use crate::bindings::core::money::{decimal_to_py, money_from_amount};
use crate::bindings::date_utils::{date_to_py, extract_date_iso};
use crate::bindings::extract::extract_basis_points;
use crate::bindings::module_utils::py_to_json_value;
use crate::bindings::pandas_utils::serde_to_py;
use crate::bindings::pickle_support::reduce_via_json;
use crate::bindings::repr_support::repr_from_serde;
use crate::errors::{core_to_py, serde_json_to_py};
use finstack_quant_calibration::api::market_datum::{
    CollateralEntry, DividendScheduleDatum, FxSpotDatum, PriceDatum,
};
use finstack_quant_calibration::quotes::cds_tranche::CdsTrancheQuote;
use finstack_quant_calibration::quotes::inflation::InflationQuote;
use finstack_quant_calibration::quotes::xccy::XccyQuote;
use finstack_quant_core::market_data::scalars::MarketScalar;
use pyo3::prelude::*;
use pyo3::IntoPyObjectExt;
use serde_json::{Map, Value};

/// Serialize `value` to compact JSON, labelling a failure with `type_name`.
fn to_json<T: serde::Serialize>(value: &T, type_name: &str) -> PyResult<String> {
    serde_json::to_string(value)
        .map_err(|e| serde_json_to_py(e, &format!("failed to serialize {type_name}")))
}

/// Strictly deserialize `json` into `T`, labelling a failure with `type_name`.
fn parse_json<T: serde::de::DeserializeOwned>(json: &str, type_name: &str) -> PyResult<T> {
    serde_json::from_str(json)
        .map_err(|e| serde_json_to_py(e, &format!("invalid {type_name} JSON")))
}

/// CDS convention wire object (``{"currency", "doc_clause"}``).
fn cds_convention(currency: &Bound<'_, PyAny>, doc_clause: &str) -> PyResult<Value> {
    let mut convention = Map::new();
    convention.insert("currency".into(), Value::String(currency_code(currency)?));
    convention.insert("doc_clause".into(), Value::String(doc_clause.into()));
    Ok(Value::Object(convention))
}

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

    fn build(
        kind: &str,
        id: &str,
        maturity: &Bound<'_, PyAny>,
        rate: f64,
        index: &str,
        convention: &str,
        frequency: Option<&str>,
    ) -> PyResult<Self> {
        let mut fields = Map::new();
        fields.insert("id".into(), Value::String(id.into()));
        fields.insert(
            "maturity".into(),
            Value::String(extract_date_iso(maturity)?),
        );
        fields.insert("rate".into(), Value::from(rate));
        fields.insert("index".into(), Value::String(index.into()));
        if let Some(frequency) = frequency {
            fields.insert("frequency".into(), Value::String(frequency.into()));
        }
        fields.insert("convention".into(), Value::String(convention.into()));
        InflationQuote::from_wire_fields(kind, fields)
            .map(Self::from_inner)
            .map_err(core_to_py)
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
        Self::build(
            "inflation_swap",
            id,
            maturity,
            rate,
            index,
            convention,
            None,
        )
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
        Self::build(
            "yoy_inflation_swap",
            id,
            maturity,
            rate,
            index,
            convention,
            Some(frequency),
        )
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
        match &self.inner {
            InflationQuote::InflationSwap { maturity, .. }
            | InflationQuote::YoYInflationSwap { maturity, .. } => date_to_py(py, *maturity),
        }
    }

    /// Fixed swap rate as a decimal.
    #[getter]
    fn rate(&self) -> f64 {
        match &self.inner {
            InflationQuote::InflationSwap { rate, .. }
            | InflationQuote::YoYInflationSwap { rate, .. } => *rate,
        }
    }

    /// Inflation index identifier.
    #[getter]
    fn index(&self) -> String {
        match &self.inner {
            InflationQuote::InflationSwap { index, .. }
            | InflationQuote::YoYInflationSwap { index, .. } => index.clone(),
        }
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
        match &self.inner {
            InflationQuote::InflationSwap { convention, .. }
            | InflationQuote::YoYInflationSwap { convention, .. } => convention.to_string(),
        }
    }

    /// Serialize to compact JSON.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If serialization fails.
    fn to_json(&self) -> PyResult<String> {
        to_json(&self.inner, "InflationQuote")
    }

    /// Rebuild from JSON produced by ``to_json``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``json`` is malformed, has unknown fields, or fails validation.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: InflationQuote = parse_json(json, "InflationQuote")?;
        inner.validate().map_err(core_to_py)?;
        Ok(Self::from_inner(inner))
    }

    /// Pickle support through the JSON wire format.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        repr_from_serde("InflationQuote", &self.inner)
    }
}

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
        let mut fields = Map::new();
        fields.insert("id".into(), Value::String(id.into()));
        fields.insert("convention".into(), Value::String(convention.into()));
        fields.insert("far_pillar".into(), extract_pillar(py, far_pillar)?);
        fields.insert(
            "basis_spread_bp".into(),
            Value::from(extract_basis_points(basis_spread_bp)?),
        );
        if let Some(spot_fx) = spot_fx {
            fields.insert("spot_fx".into(), Value::from(spot_fx));
        }
        XccyQuote::from_wire_fields(fields)
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

    /// Serialize to compact JSON.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If serialization fails.
    fn to_json(&self) -> PyResult<String> {
        to_json(&self.inner, "XccyQuote")
    }

    /// Rebuild from JSON produced by ``to_json``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``json`` is malformed, has unknown fields, or fails validation.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: XccyQuote = parse_json(json, "XccyQuote")?;
        inner.validate().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    /// Pickle support through the JSON wire format.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        repr_from_serde("XccyQuote", &self.inner)
    }
}

/// CDS index tranche quote for ``base_correlation`` calibration steps.
///
/// Examples
/// --------
/// >>> from finstack_quant.calibration import CdsTrancheQuote
/// >>> q = CdsTrancheQuote("IG-3-7", "CDX.NA.IG", 42, 0.03, 0.07, "2031-06-20", 0.01, 100.0, "USD", "isda_na")
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
    /// Build a validated CDS index tranche quote.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Unique quote identifier.
    /// index : str
    ///     Credit index name (e.g. ``"CDX.NA.IG"``).
    /// series : int
    ///     Index series number.
    /// attachment : float
    ///     Tranche attachment point as a decimal loss fraction in ``[0, 1]``.
    /// detachment : float
    ///     Tranche detachment point as a decimal loss fraction in ``[0, 1]``,
    ///     above ``attachment``.
    /// maturity : datetime.date | str
    ///     Tranche maturity date.
    /// upfront_pct : float
    ///     Upfront payment as a decimal fraction of notional
    ///     (``|upfront_pct| <= 1``).
    /// coupon_bp : float | Bps
    ///     Running coupon in basis points (> 0).
    /// currency : str | Currency
    ///     Contract currency of the CDS convention.
    /// doc_clause : str
    ///     ISDA documentation clause (``"isda_na"``, ``"cr14"``, ...).
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the date, convention or numeric inputs are invalid, or
    ///     ``attachment >= detachment``.
    #[new]
    #[pyo3(
        text_signature = "(id, index, series, attachment, detachment, maturity, upfront_pct, coupon_bp, currency, doc_clause)"
    )]
    #[allow(clippy::too_many_arguments)]
    fn new(
        id: &str,
        index: &str,
        series: u16,
        attachment: f64,
        detachment: f64,
        maturity: &Bound<'_, PyAny>,
        upfront_pct: f64,
        coupon_bp: &Bound<'_, PyAny>,
        currency: &Bound<'_, PyAny>,
        doc_clause: &str,
    ) -> PyResult<Self> {
        let mut fields = Map::new();
        fields.insert("id".into(), Value::String(id.into()));
        fields.insert("index".into(), Value::String(index.into()));
        fields.insert("series".into(), Value::from(series));
        fields.insert("attachment".into(), Value::from(attachment));
        fields.insert("detachment".into(), Value::from(detachment));
        fields.insert(
            "maturity".into(),
            Value::String(extract_date_iso(maturity)?),
        );
        fields.insert("upfront_pct".into(), Value::from(upfront_pct));
        fields.insert(
            "coupon_bp".into(),
            Value::from(extract_basis_points(coupon_bp)?),
        );
        fields.insert("convention".into(), cds_convention(currency, doc_clause)?);
        CdsTrancheQuote::from_wire_fields(fields)
            .map(|inner| Self { inner })
            .map_err(core_to_py)
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

    /// Serialize to compact JSON.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If serialization fails.
    fn to_json(&self) -> PyResult<String> {
        to_json(&self.inner, "CdsTrancheQuote")
    }

    /// Rebuild from JSON produced by ``to_json``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``json`` is malformed, has unknown fields, or fails validation.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: CdsTrancheQuote = parse_json(json, "CdsTrancheQuote")?;
        inner.validate().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    /// Pickle support through the JSON wire format.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        repr_from_serde("CdsTrancheQuote", &self.inner)
    }
}

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

    /// Serialize to compact JSON (the datum payload, without ``kind``).
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If serialization fails.
    fn to_json(&self) -> PyResult<String> {
        to_json(&self.inner, "FxSpotDatum")
    }

    /// Rebuild from JSON produced by ``to_json``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``json`` is malformed or has unknown fields.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        parse_json(json, "FxSpotDatum").map(|inner| Self { inner })
    }

    /// Pickle support through the JSON wire format.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        repr_from_serde("FxSpotDatum", &self.inner)
    }
}

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
    ///     If ``value`` is non-finite, a unitless ``Decimal`` is not exactly
    ///     representable, or the currency is unknown.
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
                MarketScalar::Price(money_from_amount(value, extract_currency(currency)?)?)
            }
            None => MarketScalar::Unitless(extract_exact_f64(value, "value")?),
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
        match &self.inner.scalar {
            MarketScalar::Unitless(value) => value.into_py_any(py),
            MarketScalar::Price(money) => Ok(decimal_to_py(py, money.amount_decimal())?.unbind()),
        }
    }

    /// Currency code of a monetary price; ``None`` when unitless.
    #[getter]
    fn currency(&self) -> Option<String> {
        match &self.inner.scalar {
            MarketScalar::Unitless(_) => None,
            MarketScalar::Price(money) => Some(money.currency().to_string()),
        }
    }

    /// Serialize to compact JSON (the datum payload, without ``kind``).
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If serialization fails.
    fn to_json(&self) -> PyResult<String> {
        to_json(&self.inner, "PriceDatum")
    }

    /// Rebuild from JSON produced by ``to_json``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``json`` is malformed or has unknown fields.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        parse_json(json, "PriceDatum").map(|inner| Self { inner })
    }

    /// Pickle support through the JSON wire format.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        repr_from_serde("PriceDatum", &self.inner)
    }
}

/// Dividend-schedule market datum (``kind == "dividend_schedule"``).
///
/// Examples
/// --------
/// >>> from finstack_quant.calibration import DividendScheduleDatum
/// >>> d = DividendScheduleDatum({"id": "AAPL-DIVS", "underlying": "AAPL", "events": []})
/// >>> d.id
/// 'AAPL-DIVS'
#[pyclass(
    name = "DividendScheduleDatum",
    module = "finstack_quant.calibration",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyDividendScheduleDatum {
    pub(crate) inner: DividendScheduleDatum,
}

#[pymethods]
impl PyDividendScheduleDatum {
    /// Build a dividend-schedule datum from the Rust ``DividendSchedule`` wire mapping.
    ///
    /// Parameters
    /// ----------
    /// schedule : dict
    ///     Dividend schedule in its serde wire form (``id``, ``underlying``,
    ///     ``events``, ...); see ``schema.get("calibration.schema.json")``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``schedule`` has a missing, unknown or mistyped field.
    #[new]
    #[pyo3(text_signature = "(schedule)")]
    fn new(py: Python<'_>, schedule: &Bound<'_, PyAny>) -> PyResult<Self> {
        let value = py_to_json_value(py, schedule, "dividend schedule")?;
        Ok(Self {
            inner: DividendScheduleDatum {
                schedule: from_value(value, "dividend schedule")?,
            },
        })
    }

    /// Identifier of the dividend schedule (the datum id).
    #[getter]
    fn id(&self) -> String {
        self.inner.schedule.get_id().as_str().to_string()
    }

    /// Dividend schedule as its serde wire mapping.
    #[getter]
    fn schedule<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.schedule)
    }

    /// Serialize to compact JSON (the datum payload, without ``kind``).
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If serialization fails.
    fn to_json(&self) -> PyResult<String> {
        to_json(&self.inner, "DividendScheduleDatum")
    }

    /// Rebuild from JSON produced by ``to_json``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``json`` is malformed or has unknown fields.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        parse_json(json, "DividendScheduleDatum").map(|inner| Self { inner })
    }

    /// Pickle support through the JSON wire format.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        repr_from_serde("DividendScheduleDatum", &self.inner)
    }
}

/// Collateral (CSA) currency mapping market datum (``kind == "collateral"``).
///
/// Examples
/// --------
/// >>> from finstack_quant.calibration import CollateralEntry
/// >>> c = CollateralEntry("EUR", "USD")
/// >>> c.id, c.csa_currency
/// ('EUR', 'USD')
#[pyclass(
    name = "CollateralEntry",
    module = "finstack_quant.calibration",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyCollateralEntry {
    pub(crate) inner: CollateralEntry,
}

#[pymethods]
impl PyCollateralEntry {
    /// Build a collateral mapping entry.
    ///
    /// Parameters
    /// ----------
    /// id : str | Currency
    ///     Trade-leg currency the CSA mapping applies to (the datum id).
    /// csa_currency : str | Currency
    ///     Collateral (CSA) currency.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a currency code is unknown.
    #[new]
    #[pyo3(text_signature = "(id, csa_currency)")]
    fn new(id: &Bound<'_, PyAny>, csa_currency: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            inner: CollateralEntry {
                id: extract_currency(id)?,
                csa_currency: extract_currency(csa_currency)?,
            },
        })
    }

    /// Trade-leg currency code (the datum id).
    #[getter]
    fn id(&self) -> String {
        self.inner.id.to_string()
    }

    /// Collateral (CSA) currency code.
    #[getter]
    fn csa_currency(&self) -> String {
        self.inner.csa_currency.to_string()
    }

    /// Serialize to compact JSON (the datum payload, without ``kind``).
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If serialization fails.
    fn to_json(&self) -> PyResult<String> {
        to_json(&self.inner, "CollateralEntry")
    }

    /// Rebuild from JSON produced by ``to_json``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``json`` is malformed or has unknown fields.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        parse_json(json, "CollateralEntry").map(|inner| Self { inner })
    }

    /// Pickle support through the JSON wire format.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        repr_from_serde("CollateralEntry", &self.inner)
    }
}
