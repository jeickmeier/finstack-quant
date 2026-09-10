//! Typed calibration-envelope authoring: quotes, steps, plans and envelopes.
//!
//! Every wrapper holds the canonical Rust value; constructors assemble the
//! serde wire shape and let the Rust deserializer validate it, so the binding
//! never re-implements field rules.

use super::config::extract_config;
use super::report::PyCalibrationValidationReport;
use crate::bindings::date_utils::extract_date_iso;
use crate::bindings::extract::{extract_basis_points, extract_rate_decimal};
use crate::bindings::module_utils::{py_to_json_value, py_to_serde};
use crate::bindings::pandas_utils::serde_to_py;
use crate::bindings::pickle_support::reduce_via_json;
use crate::bindings::repr_support::repr_from_serde;
use crate::errors::{core_to_py, serde_json_to_py, value_error};
use finstack_quant_calibration::api::market_datum::MarketDatum;
use finstack_quant_calibration::api::prior_market::PriorMarketObject;
use finstack_quant_calibration::api::schema::{
    CalibrationEnvelope, CalibrationPlan, CalibrationStep,
};
use finstack_quant_calibration::api::validate as validate_api;
use finstack_quant_calibration::quotes::cds::CdsQuote;
use finstack_quant_calibration::quotes::ids::Pillar;
use finstack_quant_calibration::quotes::rates::RateQuote;
use finstack_quant_calibration::quotes::vol::VolQuote;
use indexmap::IndexMap;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};
use serde_json::{Map, Value};

/// ISO-4217 code from a ``Currency`` object (``.code``) or a plain string.
fn currency_code(obj: &Bound<'_, PyAny>) -> PyResult<String> {
    if let Ok(code) = obj.getattr("code") {
        if let Ok(code) = code.extract::<String>() {
            return Ok(code);
        }
    }
    obj.extract::<String>().map_err(|_| {
        value_error("expected a currency code string (e.g. 'USD') or a core.Currency instance")
    })
}

/// Pillar from a tenor / ISO-date string or a ``{"tenor": ...}`` / ``{"date": ...}`` mapping.
fn extract_pillar(py: Python<'_>, obj: &Bound<'_, PyAny>) -> PyResult<Value> {
    let pillar: Pillar = if let Ok(text) = obj.extract::<String>() {
        text.parse().map_err(core_to_py)?
    } else if let Ok(date) = crate::bindings::date_utils::extract_date(obj) {
        Pillar::Date(date)
    } else {
        py_to_serde(py, obj, "pillar")?
    };
    serde_json::to_value(pillar).map_err(|e| serde_json_to_py(e, "failed to encode pillar"))
}

/// Deserialize a JSON object into `T` with a labelled error.
fn from_value<T: serde::de::DeserializeOwned>(value: Value, label: &str) -> PyResult<T> {
    serde_json::from_value(value).map_err(|e| serde_json_to_py(e, &format!("invalid {label}")))
}

/// Merge ``**params`` keyword overrides into a wire object.
fn merge_kwargs(
    py: Python<'_>,
    target: &mut Map<String, Value>,
    params: Option<&Bound<'_, PyDict>>,
    label: &str,
) -> PyResult<()> {
    if let Some(params) = params {
        for (key, value) in params.iter() {
            let key: String = key.extract()?;
            let value = if value.is_none() {
                Value::Null
            } else {
                py_to_json_value(py, &value, &format!("{label} field '{key}'"))?
            };
            target.insert(key, value);
        }
    }
    Ok(())
}

/// Interest-rate market quote (deposit, FRA, futures, or par swap) for curve calibration.
///
/// Examples
/// --------
/// >>> from finstack_quant.calibration import RateQuote
/// >>> q = RateQuote.swap("USD-OIS-SWAP-5Y", "USD-SOFR-OIS", "5Y", 0.045)
/// >>> q.id, q.type, q.value
/// ('USD-OIS-SWAP-5Y', 'swap', 0.045)
#[pyclass(
    name = "RateQuote",
    module = "finstack_quant.calibration",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyRateQuote {
    pub(crate) inner: RateQuote,
}

impl PyRateQuote {
    pub(crate) fn from_inner(inner: RateQuote) -> Self {
        Self { inner }
    }

    fn build(mut fields: Map<String, Value>, kind: &str) -> PyResult<Self> {
        fields.insert("type".to_string(), Value::String(kind.to_string()));
        let inner: RateQuote = from_value(Value::Object(fields), "RateQuote")?;
        inner.validate().map_err(core_to_py)?;
        Ok(Self::from_inner(inner))
    }
}

#[pymethods]
impl PyRateQuote {
    /// Money-market deposit quote.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Unique quote identifier (becomes the residual key in reports).
    /// index : str
    ///     Rate index identifier (e.g. ``"USD-SOFR-OIS"``).
    /// pillar : str | datetime.date | dict
    ///     Maturity pillar: tenor string (``"3M"``), ISO date / ``date``, or a
    ///     ``{"tenor": {...}}`` / ``{"date": "..."}`` mapping.
    /// rate : float | Rate
    ///     Simple deposit rate as a decimal (``0.0525`` = 5.25%).
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the pillar cannot be parsed or the rate is not finite.
    #[staticmethod]
    #[pyo3(text_signature = "(id, index, pillar, rate)")]
    fn deposit(
        py: Python<'_>,
        id: &str,
        index: &str,
        pillar: &Bound<'_, PyAny>,
        rate: &Bound<'_, PyAny>,
    ) -> PyResult<Self> {
        let mut fields = Map::new();
        fields.insert("id".into(), Value::String(id.into()));
        fields.insert("index".into(), Value::String(index.into()));
        fields.insert("pillar".into(), extract_pillar(py, pillar)?);
        fields.insert("rate".into(), Value::from(extract_rate_decimal(rate)?));
        Self::build(fields, "deposit")
    }

    /// Forward-rate-agreement quote.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Unique quote identifier.
    /// index : str
    ///     Rate index identifier of the underlying floating rate.
    /// start : str | datetime.date | dict
    ///     Accrual start pillar (tenor string, date, or pillar mapping).
    /// end : str | datetime.date | dict
    ///     Accrual end pillar.
    /// rate : float | Rate
    ///     FRA rate as a decimal.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a pillar cannot be parsed or the rate is not finite.
    #[staticmethod]
    #[pyo3(text_signature = "(id, index, start, end, rate)")]
    fn fra(
        py: Python<'_>,
        id: &str,
        index: &str,
        start: &Bound<'_, PyAny>,
        end: &Bound<'_, PyAny>,
        rate: &Bound<'_, PyAny>,
    ) -> PyResult<Self> {
        let mut fields = Map::new();
        fields.insert("id".into(), Value::String(id.into()));
        fields.insert("index".into(), Value::String(index.into()));
        fields.insert("start".into(), extract_pillar(py, start)?);
        fields.insert("end".into(), extract_pillar(py, end)?);
        fields.insert("rate".into(), Value::from(extract_rate_decimal(rate)?));
        Self::build(fields, "fra")
    }

    /// Interest-rate futures price quote.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Unique quote identifier.
    /// contract : str
    ///     Futures contract identifier (e.g. ``"CME:SR3"``).
    /// expiry : datetime.date | str
    ///     Last trading date of the contract (ISO string or ``date``).
    /// price : float
    ///     Futures price (e.g. ``98.50``); implied rate is ``(100 - price) / 100``.
    /// convexity_adjustment : float, default 0.0
    ///     Convexity adjustment as a decimal rate subtracted from the
    ///     futures-implied forward (Hull convention).
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the date cannot be parsed or the price is not finite.
    #[staticmethod]
    #[pyo3(signature = (id, contract, expiry, price, convexity_adjustment = 0.0))]
    #[pyo3(text_signature = "(id, contract, expiry, price, convexity_adjustment=0.0)")]
    fn futures(
        id: &str,
        contract: &str,
        expiry: &Bound<'_, PyAny>,
        price: f64,
        convexity_adjustment: f64,
    ) -> PyResult<Self> {
        let mut fields = Map::new();
        fields.insert("id".into(), Value::String(id.into()));
        fields.insert("contract".into(), Value::String(contract.into()));
        fields.insert("expiry".into(), Value::String(extract_date_iso(expiry)?));
        fields.insert("price".into(), Value::from(price));
        fields.insert(
            "convexity_adjustment".into(),
            Value::from(convexity_adjustment),
        );
        Self::build(fields, "futures")
    }

    /// Par swap rate quote.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Unique quote identifier.
    /// index : str
    ///     Floating-leg index identifier (e.g. ``"USD-SOFR-OIS"``).
    /// pillar : str | datetime.date | dict
    ///     Swap maturity pillar (tenor string such as ``"5Y"``, date, or mapping).
    /// rate : float | Rate
    ///     Fixed par rate as a decimal.
    /// spread_decimal : float | None, default None
    ///     Optional floating-leg spread as a decimal (``0.0010`` = 10 bp).
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the pillar cannot be parsed or the rate is not finite.
    #[staticmethod]
    #[pyo3(signature = (id, index, pillar, rate, spread_decimal = None))]
    #[pyo3(text_signature = "(id, index, pillar, rate, spread_decimal=None)")]
    fn swap(
        py: Python<'_>,
        id: &str,
        index: &str,
        pillar: &Bound<'_, PyAny>,
        rate: &Bound<'_, PyAny>,
        spread_decimal: Option<f64>,
    ) -> PyResult<Self> {
        let mut fields = Map::new();
        fields.insert("id".into(), Value::String(id.into()));
        fields.insert("index".into(), Value::String(index.into()));
        fields.insert("pillar".into(), extract_pillar(py, pillar)?);
        fields.insert("rate".into(), Value::from(extract_rate_decimal(rate)?));
        fields.insert(
            "spread_decimal".into(),
            spread_decimal.map_or(Value::Null, Value::from),
        );
        Self::build(fields, "swap")
    }

    /// Unique quote identifier.
    #[getter]
    fn id(&self) -> String {
        self.inner.id().as_str().to_string()
    }

    /// Quote type: ``"deposit"``, ``"fra"``, ``"futures"`` or ``"swap"``.
    #[getter]
    #[pyo3(name = "type")]
    fn quote_type(&self) -> &'static str {
        match self.inner {
            RateQuote::Deposit { .. } => "deposit",
            RateQuote::Fra { .. } => "fra",
            RateQuote::Futures { .. } => "futures",
            RateQuote::Swap { .. } => "swap",
        }
    }

    /// Quoted value (decimal rate, or price for futures).
    #[getter]
    fn value(&self) -> f64 {
        self.inner.value()
    }

    /// Rate implied by the quote as a decimal (futures price converted).
    #[getter]
    fn implied_rate(&self) -> f64 {
        self.inner.implied_rate()
    }

    /// Serialize to compact JSON (``{"type": ..., "id": ..., ...}``).
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If serialization fails.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|e| serde_json_to_py(e, "failed to serialize RateQuote"))
    }

    /// Rebuild from JSON produced by ``to_json``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``json`` is malformed, has unknown fields, or fails validation.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: RateQuote = serde_json::from_str(json)
            .map_err(|e| serde_json_to_py(e, "invalid RateQuote JSON"))?;
        inner.validate().map_err(core_to_py)?;
        Ok(Self::from_inner(inner))
    }

    /// Pickle support through the JSON wire format.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        repr_from_serde("RateQuote", &self.inner)
    }
}

/// Single-name CDS quote (par spread or upfront) for hazard-curve calibration.
///
/// Examples
/// --------
/// >>> from finstack_quant.calibration import CdsQuote
/// >>> q = CdsQuote.par_spread("ACME-5Y", "ACME", "USD", "isda_na", "5Y", 80.0, 0.4)
/// >>> q.id, q.type
/// ('ACME-5Y', 'cds_par_spread')
#[pyclass(
    name = "CdsQuote",
    module = "finstack_quant.calibration",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyCdsQuote {
    pub(crate) inner: CdsQuote,
}

impl PyCdsQuote {
    pub(crate) fn from_inner(inner: CdsQuote) -> Self {
        Self { inner }
    }

    #[allow(clippy::too_many_arguments)]
    fn build(
        py: Python<'_>,
        kind: &str,
        id: &str,
        entity: &str,
        currency: &Bound<'_, PyAny>,
        doc_clause: &str,
        pillar: &Bound<'_, PyAny>,
        extra: Vec<(&str, f64)>,
    ) -> PyResult<Self> {
        let mut fields = Map::new();
        fields.insert("type".into(), Value::String(kind.into()));
        fields.insert("id".into(), Value::String(id.into()));
        fields.insert("entity".into(), Value::String(entity.into()));
        let mut convention = Map::new();
        convention.insert("currency".into(), Value::String(currency_code(currency)?));
        convention.insert("doc_clause".into(), Value::String(doc_clause.into()));
        fields.insert("convention".into(), Value::Object(convention));
        fields.insert("pillar".into(), extract_pillar(py, pillar)?);
        for (key, value) in extra {
            fields.insert(key.into(), Value::from(value));
        }
        let inner: CdsQuote = from_value(Value::Object(fields), "CdsQuote")?;
        inner.validate().map_err(core_to_py)?;
        Ok(Self::from_inner(inner))
    }
}

#[pymethods]
impl PyCdsQuote {
    /// Running par-spread CDS quote.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Unique quote identifier.
    /// entity : str
    ///     Reference entity name.
    /// currency : str | Currency
    ///     Contract currency of the CDS convention.
    /// doc_clause : str
    ///     ISDA documentation clause (``"isda_na"``, ``"isda_eu"``, ``"cr14"``,
    ///     ``"mr14"``, ``"mm14"``, ``"xr14"``).
    /// pillar : str | datetime.date | dict
    ///     Maturity pillar (tenor string, date, or pillar mapping).
    /// spread_bp : float | Bps
    ///     Par spread in basis points.
    /// recovery_rate : float
    ///     Assumed recovery rate as a decimal in ``[0, 1)``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the convention, pillar or numeric inputs are invalid.
    #[staticmethod]
    #[pyo3(text_signature = "(id, entity, currency, doc_clause, pillar, spread_bp, recovery_rate)")]
    #[allow(clippy::too_many_arguments)]
    fn par_spread(
        py: Python<'_>,
        id: &str,
        entity: &str,
        currency: &Bound<'_, PyAny>,
        doc_clause: &str,
        pillar: &Bound<'_, PyAny>,
        spread_bp: &Bound<'_, PyAny>,
        recovery_rate: f64,
    ) -> PyResult<Self> {
        Self::build(
            py,
            "cds_par_spread",
            id,
            entity,
            currency,
            doc_clause,
            pillar,
            vec![
                ("spread_bp", extract_basis_points(spread_bp)?),
                ("recovery_rate", recovery_rate),
            ],
        )
    }

    /// Upfront-plus-running-coupon CDS quote.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Unique quote identifier.
    /// entity : str
    ///     Reference entity name.
    /// currency : str | Currency
    ///     Contract currency of the CDS convention.
    /// doc_clause : str
    ///     ISDA documentation clause (see ``par_spread``).
    /// pillar : str | datetime.date | dict
    ///     Maturity pillar.
    /// running_spread_bp : float | Bps
    ///     Standard running coupon in basis points (e.g. ``100.0``).
    /// upfront_pct : float
    ///     Upfront payment as a percentage of notional (points).
    /// recovery_rate : float
    ///     Assumed recovery rate as a decimal in ``[0, 1)``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the convention, pillar or numeric inputs are invalid.
    #[staticmethod]
    #[pyo3(
        text_signature = "(id, entity, currency, doc_clause, pillar, running_spread_bp, upfront_pct, recovery_rate)"
    )]
    #[allow(clippy::too_many_arguments)]
    fn upfront(
        py: Python<'_>,
        id: &str,
        entity: &str,
        currency: &Bound<'_, PyAny>,
        doc_clause: &str,
        pillar: &Bound<'_, PyAny>,
        running_spread_bp: &Bound<'_, PyAny>,
        upfront_pct: f64,
        recovery_rate: f64,
    ) -> PyResult<Self> {
        Self::build(
            py,
            "cds_upfront",
            id,
            entity,
            currency,
            doc_clause,
            pillar,
            vec![
                (
                    "running_spread_bp",
                    extract_basis_points(running_spread_bp)?,
                ),
                ("upfront_pct", upfront_pct),
                ("recovery_rate", recovery_rate),
            ],
        )
    }

    /// Unique quote identifier.
    #[getter]
    fn id(&self) -> String {
        self.inner.id().as_str().to_string()
    }

    /// Quote type: ``"cds_par_spread"`` or ``"cds_upfront"``.
    #[getter]
    #[pyo3(name = "type")]
    fn quote_type(&self) -> &'static str {
        match self.inner {
            CdsQuote::CdsParSpread { .. } => "cds_par_spread",
            CdsQuote::CdsUpfront { .. } => "cds_upfront",
        }
    }

    /// Quoted running spread in basis points.
    #[getter]
    fn running_spread_bp(&self) -> f64 {
        self.inner.quoted_running_spread_bp()
    }

    /// Serialize to compact JSON.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If serialization fails.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|e| serde_json_to_py(e, "failed to serialize CdsQuote"))
    }

    /// Rebuild from JSON produced by ``to_json``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``json`` is malformed, has unknown fields, or fails validation.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: CdsQuote =
            serde_json::from_str(json).map_err(|e| serde_json_to_py(e, "invalid CdsQuote JSON"))?;
        inner.validate().map_err(core_to_py)?;
        Ok(Self::from_inner(inner))
    }

    /// Pickle support through the JSON wire format.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        repr_from_serde("CdsQuote", &self.inner)
    }
}

/// Volatility quote (equity option, swaption, or cap/floor) for surface calibration.
///
/// Examples
/// --------
/// >>> from finstack_quant.calibration import VolQuote
/// >>> q = VolQuote.option_vol("AAPL-1Y-155C", "AAPL", "2027-05-08", 155.0, 0.28, "call")
/// >>> q.id, q.type, q.volatility
/// ('AAPL-1Y-155C', 'option_vol', 0.28)
#[pyclass(
    name = "VolQuote",
    module = "finstack_quant.calibration",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyVolQuote {
    pub(crate) inner: VolQuote,
}

impl PyVolQuote {
    pub(crate) fn from_inner(inner: VolQuote) -> Self {
        Self { inner }
    }

    fn build(variant: &str, fields: Map<String, Value>) -> PyResult<Self> {
        let mut outer = Map::new();
        outer.insert(variant.to_string(), Value::Object(fields));
        let inner: VolQuote = from_value(Value::Object(outer), "VolQuote")?;
        inner.validate().map_err(core_to_py)?;
        Ok(Self::from_inner(inner))
    }
}

#[pymethods]
impl PyVolQuote {
    /// Listed equity/FX option implied-volatility quote.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Unique quote identifier.
    /// underlying : str
    ///     Underlying identifier (ticker) the surface is keyed by.
    /// expiry : datetime.date | str
    ///     Option expiry date.
    /// strike : float
    ///     Absolute strike in underlying price units.
    /// vol : float
    ///     Black implied volatility as an annualized decimal.
    /// option_type : str, default "call"
    ///     ``"call"`` or ``"put"``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the date or numeric inputs are invalid.
    #[staticmethod]
    #[pyo3(signature = (id, underlying, expiry, strike, vol, option_type = "call"))]
    #[pyo3(text_signature = "(id, underlying, expiry, strike, vol, option_type='call')")]
    fn option_vol(
        id: &str,
        underlying: &str,
        expiry: &Bound<'_, PyAny>,
        strike: f64,
        vol: f64,
        option_type: &str,
    ) -> PyResult<Self> {
        let mut fields = Map::new();
        fields.insert("id".into(), Value::String(id.into()));
        fields.insert("underlying".into(), Value::String(underlying.into()));
        fields.insert("expiry".into(), Value::String(extract_date_iso(expiry)?));
        fields.insert("strike".into(), Value::from(strike));
        fields.insert("vol".into(), Value::from(vol));
        fields.insert("option_type".into(), Value::String(option_type.into()));
        Self::build("option_vol", fields)
    }

    /// Swaption volatility quote.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Unique quote identifier.
    /// expiry : datetime.date | str
    ///     Option expiry date.
    /// maturity : datetime.date | str
    ///     Underlying swap maturity date.
    /// strike : float
    ///     Fixed strike rate as a decimal.
    /// vol : float
    ///     Volatility: annualized decimal (lognormal) or absolute rate
    ///     volatility (normal, e.g. ``0.0072``).
    /// quote_type : str, default "normal"
    ///     ``"normal"`` or ``"black_lognormal"``.
    /// convention : str, default "USD"
    ///     Swaption market convention identifier.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a date or numeric input is invalid.
    #[staticmethod]
    #[pyo3(signature = (id, expiry, maturity, strike, vol, quote_type = "normal", convention = "USD"))]
    #[pyo3(
        text_signature = "(id, expiry, maturity, strike, vol, quote_type='normal', convention='USD')"
    )]
    #[allow(clippy::too_many_arguments)]
    fn swaption_vol(
        id: &str,
        expiry: &Bound<'_, PyAny>,
        maturity: &Bound<'_, PyAny>,
        strike: f64,
        vol: f64,
        quote_type: &str,
        convention: &str,
    ) -> PyResult<Self> {
        let mut fields = Map::new();
        fields.insert("id".into(), Value::String(id.into()));
        fields.insert("expiry".into(), Value::String(extract_date_iso(expiry)?));
        fields.insert(
            "maturity".into(),
            Value::String(extract_date_iso(maturity)?),
        );
        fields.insert("strike".into(), Value::from(strike));
        fields.insert("vol".into(), Value::from(vol));
        fields.insert("quote_type".into(), Value::String(quote_type.into()));
        fields.insert("convention".into(), Value::String(convention.into()));
        Self::build("swaption_vol", fields)
    }

    /// Cap or floor volatility quote.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Unique quote identifier.
    /// expiry : datetime.date | str
    ///     Cap/floor maturity date.
    /// strike : float
    ///     Strike rate as a decimal.
    /// vol : float
    ///     Volatility (normal absolute or lognormal decimal per ``quote_type``).
    /// quote_type : str, default "normal"
    ///     ``"normal"`` or ``"black_lognormal"``.
    /// is_cap : bool, default True
    ///     ``True`` for a cap, ``False`` for a floor.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the date or numeric inputs are invalid.
    #[staticmethod]
    #[pyo3(signature = (id, expiry, strike, vol, quote_type = "normal", is_cap = true))]
    #[pyo3(text_signature = "(id, expiry, strike, vol, quote_type='normal', is_cap=True)")]
    fn cap_floor_vol(
        id: &str,
        expiry: &Bound<'_, PyAny>,
        strike: f64,
        vol: f64,
        quote_type: &str,
        is_cap: bool,
    ) -> PyResult<Self> {
        let mut fields = Map::new();
        fields.insert("id".into(), Value::String(id.into()));
        fields.insert("expiry".into(), Value::String(extract_date_iso(expiry)?));
        fields.insert("strike".into(), Value::from(strike));
        fields.insert("vol".into(), Value::from(vol));
        fields.insert("quote_type".into(), Value::String(quote_type.into()));
        fields.insert("is_cap".into(), Value::Bool(is_cap));
        Self::build("cap_floor_vol", fields)
    }

    /// Unique quote identifier.
    #[getter]
    fn id(&self) -> String {
        self.inner.id().as_str().to_string()
    }

    /// Quote type: ``"option_vol"``, ``"swaption_vol"`` or ``"cap_floor_vol"``.
    #[getter]
    #[pyo3(name = "type")]
    fn quote_type(&self) -> &'static str {
        match self.inner {
            VolQuote::OptionVol { .. } => "option_vol",
            VolQuote::SwaptionVol { .. } => "swaption_vol",
            VolQuote::CapFloorVol { .. } => "cap_floor_vol",
        }
    }

    /// Quoted volatility.
    #[getter]
    fn volatility(&self) -> f64 {
        self.inner.volatility()
    }

    /// Serialize to compact JSON.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If serialization fails.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|e| serde_json_to_py(e, "failed to serialize VolQuote"))
    }

    /// Rebuild from JSON produced by ``to_json``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``json`` is malformed, has unknown fields, or fails validation.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: VolQuote =
            serde_json::from_str(json).map_err(|e| serde_json_to_py(e, "invalid VolQuote JSON"))?;
        inner.validate().map_err(core_to_py)?;
        Ok(Self::from_inner(inner))
    }

    /// Pickle support through the JSON wire format.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        repr_from_serde("VolQuote", &self.inner)
    }
}

/// Convert one market-data entry (typed quote, dict, or JSON string) into a `MarketDatum`.
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
    let value = py_to_json_value(py, obj, "market datum")?;
    from_value(value, "market datum (expected {\"kind\": ..., ...})")
}

fn extract_market_data(
    py: Python<'_>,
    obj: Option<&Bound<'_, PyAny>>,
) -> PyResult<Vec<MarketDatum>> {
    let Some(obj) = obj else {
        return Ok(Vec::new());
    };
    if obj.is_none() {
        return Ok(Vec::new());
    }
    obj.try_iter()?
        .map(|item| extract_market_datum(py, &item?))
        .collect()
}

fn extract_prior_market(
    py: Python<'_>,
    obj: Option<&Bound<'_, PyAny>>,
) -> PyResult<Vec<PriorMarketObject>> {
    let Some(obj) = obj else {
        return Ok(Vec::new());
    };
    if obj.is_none() {
        return Ok(Vec::new());
    }
    obj.try_iter()?
        .map(|item| {
            let value = py_to_json_value(py, &item?, "prior market object")?;
            from_value(value, "prior market object (expected {\"kind\": ..., ...})")
        })
        .collect()
}

/// One calibration step: the object to build, its quote set, and kind-specific parameters.
///
/// Every constructor takes the step ``id`` and the kind's required fields;
/// remaining optional fields are keyword overrides named exactly as in the
/// Rust ``StepParams`` wire schema (see ``schema.get("calibration.schema.json")``).
/// Quotes may be attached directly (``quotes=[...]``): the plan then derives
/// the quote set from them.
///
/// Examples
/// --------
/// >>> from finstack_quant.calibration import CalibrationStep, RateQuote
/// >>> quotes = [RateQuote.deposit("D3M", "USD-SOFR-OIS", "3M", 0.052)]
/// >>> step = CalibrationStep.discount("USD-OIS", "USD", "2026-05-08", quotes=quotes)
/// >>> step.kind, step.quote_set, step.quote_ids
/// ('discount', 'USD-OIS', ['D3M'])
#[pyclass(
    name = "CalibrationStep",
    module = "finstack_quant.calibration",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyCalibrationStep {
    pub(crate) inner: CalibrationStep,
    /// Quotes attached at construction; the plan derives a quote set from them.
    pub(crate) quotes: Vec<MarketDatum>,
}

impl PyCalibrationStep {
    pub(crate) fn from_inner(inner: CalibrationStep) -> Self {
        Self {
            inner,
            quotes: Vec::new(),
        }
    }

    /// Shared constructor: `kind` + `id` + explicit fields + `**params`.
    fn build(
        py: Python<'_>,
        kind: &str,
        id: &str,
        quotes: Option<&Bound<'_, PyAny>>,
        quote_set: Option<String>,
        mut fields: Map<String, Value>,
        params: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        merge_kwargs(py, &mut fields, params, kind)?;
        fields.insert("kind".into(), Value::String(kind.into()));
        fields.insert("id".into(), Value::String(id.into()));
        fields.insert(
            "quote_set".into(),
            Value::String(quote_set.unwrap_or_else(|| id.to_string())),
        );
        let inner: CalibrationStep = from_value(Value::Object(fields), &format!("{kind} step"))?;
        Ok(Self {
            inner,
            quotes: extract_market_data(py, quotes)?,
        })
    }
}

/// Insert `key: value` pairs into a wire map.
fn fields(pairs: Vec<(&str, Value)>) -> Map<String, Value> {
    pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect()
}

#[pymethods]
impl PyCalibrationStep {
    /// Discount-curve bootstrap step.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Step identifier (also the default quote-set name and, unless
    ///     ``curve_id`` is given, the curve identifier).
    /// currency : str | Currency
    ///     Curve currency.
    /// base_date : datetime.date | str
    ///     Curve base (valuation) date.
    /// quotes : list[RateQuote | dict] | None, default None
    ///     Quotes to attach; the plan builds the quote set from their ids.
    /// quote_set : str | None, default None
    ///     Quote-set name in ``plan.quote_sets``; defaults to ``id``.
    /// curve_id : str | None, default None
    ///     Identifier of the produced curve; defaults to ``id``.
    /// **params
    ///     Optional wire fields: ``method`` (``"bootstrap"`` | ``"global"``),
    ///     ``interpolation`` (``"log_linear"`` default, ``"linear"``,
    ///     ``"monotone_convex"``, ...), ``extrapolation`` (``"flat_forward"``),
    ///     ``pricing_discount_id``, ``pricing_forward_id``, ``conventions``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a field is unknown or has the wrong shape.
    #[staticmethod]
    #[pyo3(signature = (id, currency, base_date, quotes = None, quote_set = None, curve_id = None, **params))]
    #[pyo3(
        text_signature = "(id, currency, base_date, quotes=None, quote_set=None, curve_id=None, **params)"
    )]
    #[allow(clippy::too_many_arguments)]
    fn discount(
        py: Python<'_>,
        id: &str,
        currency: &Bound<'_, PyAny>,
        base_date: &Bound<'_, PyAny>,
        quotes: Option<&Bound<'_, PyAny>>,
        quote_set: Option<String>,
        curve_id: Option<String>,
        params: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        let f = fields(vec![
            (
                "curve_id",
                Value::String(curve_id.unwrap_or_else(|| id.into())),
            ),
            ("currency", Value::String(currency_code(currency)?)),
            ("base_date", Value::String(extract_date_iso(base_date)?)),
        ]);
        Self::build(py, "discount", id, quotes, quote_set, f, params)
    }

    /// Forward (index projection) curve step discounted on an existing curve.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Step identifier (default quote-set name and curve id).
    /// currency : str | Currency
    ///     Curve currency.
    /// base_date : datetime.date | str
    ///     Curve base date.
    /// tenor_years : float
    ///     Index accrual tenor in years (``0.25`` for 3M).
    /// discount_curve_id : str
    ///     Identifier of the discount curve used to price the quotes.
    /// quotes, quote_set, curve_id
    ///     As in ``discount``.
    /// **params
    ///     Optional wire fields: ``method``, ``interpolation``
    ///     (``"monotone_convex"`` default), ``conventions``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a field is unknown or has the wrong shape.
    #[staticmethod]
    #[pyo3(signature = (id, currency, base_date, tenor_years, discount_curve_id, quotes = None, quote_set = None, curve_id = None, **params))]
    #[pyo3(
        text_signature = "(id, currency, base_date, tenor_years, discount_curve_id, quotes=None, quote_set=None, curve_id=None, **params)"
    )]
    #[allow(clippy::too_many_arguments)]
    fn forward(
        py: Python<'_>,
        id: &str,
        currency: &Bound<'_, PyAny>,
        base_date: &Bound<'_, PyAny>,
        tenor_years: f64,
        discount_curve_id: &str,
        quotes: Option<&Bound<'_, PyAny>>,
        quote_set: Option<String>,
        curve_id: Option<String>,
        params: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        let f = fields(vec![
            (
                "curve_id",
                Value::String(curve_id.unwrap_or_else(|| id.into())),
            ),
            ("currency", Value::String(currency_code(currency)?)),
            ("base_date", Value::String(extract_date_iso(base_date)?)),
            ("tenor_years", Value::from(tenor_years)),
            ("discount_curve_id", Value::String(discount_curve_id.into())),
        ]);
        Self::build(py, "forward", id, quotes, quote_set, f, params)
    }

    /// Hazard-curve bootstrap step from CDS quotes.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Step identifier (default quote-set name and curve id).
    /// entity : str
    ///     Reference entity.
    /// currency : str | Currency
    ///     Curve currency.
    /// base_date : datetime.date | str
    ///     Curve base date.
    /// discount_curve_id : str
    ///     Discount curve used for CDS present values.
    /// recovery_rate : float
    ///     Assumed recovery rate as a decimal.
    /// seniority : str, default "senior"
    ///     Debt seniority (``"senior"``, ``"subordinated"``, ...).
    /// quotes, quote_set, curve_id
    ///     As in ``discount``.
    /// **params
    ///     Optional wire fields: ``notional``, ``method``, ``interpolation``,
    ///     ``par_interp``, ``doc_clause``, ``cds_valuation_convention``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a field is unknown or has the wrong shape.
    #[staticmethod]
    #[pyo3(signature = (id, entity, currency, base_date, discount_curve_id, recovery_rate, seniority = "senior", quotes = None, quote_set = None, curve_id = None, **params))]
    #[pyo3(
        text_signature = "(id, entity, currency, base_date, discount_curve_id, recovery_rate, seniority='senior', quotes=None, quote_set=None, curve_id=None, **params)"
    )]
    #[allow(clippy::too_many_arguments)]
    fn hazard(
        py: Python<'_>,
        id: &str,
        entity: &str,
        currency: &Bound<'_, PyAny>,
        base_date: &Bound<'_, PyAny>,
        discount_curve_id: &str,
        recovery_rate: f64,
        seniority: &str,
        quotes: Option<&Bound<'_, PyAny>>,
        quote_set: Option<String>,
        curve_id: Option<String>,
        params: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        let f = fields(vec![
            (
                "curve_id",
                Value::String(curve_id.unwrap_or_else(|| id.into())),
            ),
            ("entity", Value::String(entity.into())),
            ("seniority", Value::String(seniority.into())),
            ("currency", Value::String(currency_code(currency)?)),
            ("base_date", Value::String(extract_date_iso(base_date)?)),
            ("discount_curve_id", Value::String(discount_curve_id.into())),
            ("recovery_rate", Value::from(recovery_rate)),
        ]);
        Self::build(py, "hazard", id, quotes, quote_set, f, params)
    }

    /// Inflation (CPI projection) curve step from inflation-swap quotes.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Step identifier (default quote-set name and curve id).
    /// currency : str | Currency
    ///     Curve currency.
    /// base_date : datetime.date | str
    ///     Curve base date.
    /// discount_curve_id : str
    ///     Discount curve used for swap present values.
    /// index : str
    ///     Inflation index identifier (e.g. ``"USA-CPI-U"``).
    /// observation_lag : str
    ///     Observation lag tenor (e.g. ``"3M"``).
    /// base_cpi : float
    ///     CPI level at the base date.
    /// quotes, quote_set, curve_id
    ///     As in ``discount``.
    /// **params
    ///     Optional wire fields: ``notional``, ``method``, ``interpolation``,
    ///     ``seasonal_factors``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a field is unknown or has the wrong shape.
    #[staticmethod]
    #[pyo3(signature = (id, currency, base_date, discount_curve_id, index, observation_lag, base_cpi, quotes = None, quote_set = None, curve_id = None, **params))]
    #[pyo3(
        text_signature = "(id, currency, base_date, discount_curve_id, index, observation_lag, base_cpi, quotes=None, quote_set=None, curve_id=None, **params)"
    )]
    #[allow(clippy::too_many_arguments)]
    fn inflation(
        py: Python<'_>,
        id: &str,
        currency: &Bound<'_, PyAny>,
        base_date: &Bound<'_, PyAny>,
        discount_curve_id: &str,
        index: &str,
        observation_lag: &str,
        base_cpi: f64,
        quotes: Option<&Bound<'_, PyAny>>,
        quote_set: Option<String>,
        curve_id: Option<String>,
        params: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        let f = fields(vec![
            (
                "curve_id",
                Value::String(curve_id.unwrap_or_else(|| id.into())),
            ),
            ("currency", Value::String(currency_code(currency)?)),
            ("base_date", Value::String(extract_date_iso(base_date)?)),
            ("discount_curve_id", Value::String(discount_curve_id.into())),
            ("index", Value::String(index.into())),
            ("observation_lag", Value::String(observation_lag.into())),
            ("base_cpi", Value::from(base_cpi)),
        ]);
        Self::build(py, "inflation", id, quotes, quote_set, f, params)
    }

    /// Equity/FX volatility-surface (SABR) step from option vol quotes.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Step identifier (default quote-set name and surface id).
    /// base_date : datetime.date | str
    ///     Surface base date.
    /// underlying_ticker : str
    ///     Underlying identifier the quotes reference.
    /// model : str, default "sabr"
    ///     Surface model.
    /// quotes, quote_set
    ///     As in ``discount``.
    /// vol_surface_id : str | None, default None
    ///     Identifier of the produced surface; defaults to ``id``.
    /// **params
    ///     Optional wire fields: ``discount_curve_id``, ``beta``,
    ///     ``target_expiries``, ``target_strikes``, ``spot_override``,
    ///     ``dividend_yield_override``, ``expiry_extrapolation``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a field is unknown or has the wrong shape.
    #[staticmethod]
    #[pyo3(signature = (id, base_date, underlying_ticker, model = "sabr", quotes = None, quote_set = None, vol_surface_id = None, **params))]
    #[pyo3(
        text_signature = "(id, base_date, underlying_ticker, model='sabr', quotes=None, quote_set=None, vol_surface_id=None, **params)"
    )]
    #[allow(clippy::too_many_arguments)]
    fn vol_surface(
        py: Python<'_>,
        id: &str,
        base_date: &Bound<'_, PyAny>,
        underlying_ticker: &str,
        model: &str,
        quotes: Option<&Bound<'_, PyAny>>,
        quote_set: Option<String>,
        vol_surface_id: Option<String>,
        params: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        let f = fields(vec![
            (
                "vol_surface_id",
                Value::String(vol_surface_id.unwrap_or_else(|| id.into())),
            ),
            ("base_date", Value::String(extract_date_iso(base_date)?)),
            ("underlying_ticker", Value::String(underlying_ticker.into())),
            ("model", Value::String(model.into())),
        ]);
        Self::build(py, "vol_surface", id, quotes, quote_set, f, params)
    }

    /// Swaption volatility cube step from swaption vol quotes.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Step identifier (default quote-set name and surface id).
    /// base_date : datetime.date | str
    ///     Surface base date.
    /// discount_curve_id : str
    ///     Discount curve for forward-swap-rate construction.
    /// currency : str | Currency
    ///     Surface currency.
    /// quotes, quote_set, vol_surface_id
    ///     As in ``vol_surface``.
    /// **params
    ///     Optional wire fields: ``forward_id``, ``vol_convention``,
    ///     ``sabr_beta``, ``target_expiries``, ``target_tenors``,
    ///     ``sabr_interpolation``, ``calendar_id``, ``fixed_day_count``,
    ///     ``swap_index``, ``vol_tolerance``,
    ///     ``sabr_extrapolation``, ``allow_sabr_missing_bucket_fallback``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a field is unknown or has the wrong shape.
    #[staticmethod]
    #[pyo3(signature = (id, base_date, discount_curve_id, currency, quotes = None, quote_set = None, vol_surface_id = None, **params))]
    #[pyo3(
        text_signature = "(id, base_date, discount_curve_id, currency, quotes=None, quote_set=None, vol_surface_id=None, **params)"
    )]
    #[allow(clippy::too_many_arguments)]
    fn swaption_vol(
        py: Python<'_>,
        id: &str,
        base_date: &Bound<'_, PyAny>,
        discount_curve_id: &str,
        currency: &Bound<'_, PyAny>,
        quotes: Option<&Bound<'_, PyAny>>,
        quote_set: Option<String>,
        vol_surface_id: Option<String>,
        params: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        let f = fields(vec![
            (
                "vol_surface_id",
                Value::String(vol_surface_id.unwrap_or_else(|| id.into())),
            ),
            ("base_date", Value::String(extract_date_iso(base_date)?)),
            ("discount_curve_id", Value::String(discount_curve_id.into())),
            ("currency", Value::String(currency_code(currency)?)),
        ]);
        Self::build(py, "swaption_vol", id, quotes, quote_set, f, params)
    }

    /// Index-tranche base-correlation step.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Step identifier (default quote-set name).
    /// index_id : str
    ///     Credit index identifier (the curve is written as ``"{index_id}_CORR"``).
    /// series : int
    ///     Index series number.
    /// maturity_years : float
    ///     Tranche maturity in years.
    /// base_date : datetime.date | str
    ///     Valuation date.
    /// discount_curve_id : str
    ///     Discount curve for tranche present values.
    /// currency : str | Currency
    ///     Index currency.
    /// quotes, quote_set
    ///     As in ``discount``.
    /// **params
    ///     Optional wire fields: ``notional``, ``frequency``, ``day_count``,
    ///     ``business_day_convention``, ``calendar_id``, ``detachment_points``,
    ///     ``use_imm_dates``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a field is unknown or has the wrong shape.
    #[staticmethod]
    #[pyo3(signature = (id, index_id, series, maturity_years, base_date, discount_curve_id, currency, quotes = None, quote_set = None, **params))]
    #[pyo3(
        text_signature = "(id, index_id, series, maturity_years, base_date, discount_curve_id, currency, quotes=None, quote_set=None, **params)"
    )]
    #[allow(clippy::too_many_arguments)]
    fn base_correlation(
        py: Python<'_>,
        id: &str,
        index_id: &str,
        series: u16,
        maturity_years: f64,
        base_date: &Bound<'_, PyAny>,
        discount_curve_id: &str,
        currency: &Bound<'_, PyAny>,
        quotes: Option<&Bound<'_, PyAny>>,
        quote_set: Option<String>,
        params: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        let f = fields(vec![
            ("index_id", Value::String(index_id.into())),
            ("series", Value::from(series)),
            ("maturity_years", Value::from(maturity_years)),
            ("base_date", Value::String(extract_date_iso(base_date)?)),
            ("discount_curve_id", Value::String(discount_curve_id.into())),
            ("currency", Value::String(currency_code(currency)?)),
        ]);
        Self::build(py, "base_correlation", id, quotes, quote_set, f, params)
    }

    /// Student-t copula degrees-of-freedom step for one tranche.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Step identifier (default quote-set name).
    /// tranche_instrument_id : str
    ///     Tranche instrument whose ``"{id}_STUDENT_T_DF"`` scalar is written.
    /// base_correlation_curve_id : str
    ///     Base-correlation curve the tranche is priced on.
    /// quotes, quote_set
    ///     As in ``discount``.
    /// **params
    ///     Optional wire fields: ``discount_curve_id``, ``initial_df``,
    ///     ``df_bounds``, ``correlation``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a field is unknown or has the wrong shape.
    #[staticmethod]
    #[pyo3(signature = (id, tranche_instrument_id, base_correlation_curve_id, quotes = None, quote_set = None, **params))]
    #[pyo3(
        text_signature = "(id, tranche_instrument_id, base_correlation_curve_id, quotes=None, quote_set=None, **params)"
    )]
    fn student_t(
        py: Python<'_>,
        id: &str,
        tranche_instrument_id: &str,
        base_correlation_curve_id: &str,
        quotes: Option<&Bound<'_, PyAny>>,
        quote_set: Option<String>,
        params: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        let f = fields(vec![
            (
                "tranche_instrument_id",
                Value::String(tranche_instrument_id.into()),
            ),
            (
                "base_correlation_curve_id",
                Value::String(base_correlation_curve_id.into()),
            ),
        ]);
        Self::build(py, "student_t", id, quotes, quote_set, f, params)
    }

    /// Hull-White one-factor calibration to swaption quotes.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Step identifier (default quote-set name).
    /// curve_id : str
    ///     Discount curve the model is calibrated on (scalars are written as
    ///     ``"{curve_id}_HW1F"``).
    /// currency : str | Currency
    ///     Model currency.
    /// base_date : datetime.date | str
    ///     Valuation date.
    /// quotes, quote_set
    ///     ATM swaption quotes; calibration rejects strikes differing from the
    ///     contractual forward by more than 1e-8 in decimal rate units.
    /// **params
    ///     Optional wire fields: ``initial_kappa``, ``initial_sigma``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a field is unknown or has the wrong shape.
    #[staticmethod]
    #[pyo3(signature = (id, curve_id, currency, base_date, quotes = None, quote_set = None, **params))]
    #[pyo3(
        text_signature = "(id, curve_id, currency, base_date, quotes=None, quote_set=None, **params)"
    )]
    #[allow(clippy::too_many_arguments)]
    fn hull_white(
        py: Python<'_>,
        id: &str,
        curve_id: &str,
        currency: &Bound<'_, PyAny>,
        base_date: &Bound<'_, PyAny>,
        quotes: Option<&Bound<'_, PyAny>>,
        quote_set: Option<String>,
        params: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        let f = fields(vec![
            ("curve_id", Value::String(curve_id.into())),
            ("currency", Value::String(currency_code(currency)?)),
            ("base_date", Value::String(extract_date_iso(base_date)?)),
        ]);
        Self::build(py, "hull_white", id, quotes, quote_set, f, params)
    }

    /// Hull-White one-factor calibration to cap/floor quotes.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Step identifier (default quote-set name).
    /// discount_curve_id : str
    ///     Discounting curve (scalars are written as ``"{discount_curve_id}_CAPFLOOR_HW1F"``).
    /// forward_curve_id : str
    ///     Curve projecting the caplet forwards.
    /// currency : str | Currency
    ///     Model currency.
    /// base_date : datetime.date | str
    ///     Valuation date.
    /// quotes, quote_set
    ///     As in ``discount``.
    /// **params
    ///     Optional wire fields: ``fixed_kappa``, ``initial_kappa``,
    ///     ``initial_sigma``, ``payment_frequency``, ``volatility_mode``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a field is unknown or has the wrong shape.
    #[staticmethod]
    #[pyo3(signature = (id, discount_curve_id, forward_curve_id, currency, base_date, quotes = None, quote_set = None, **params))]
    #[pyo3(
        text_signature = "(id, discount_curve_id, forward_curve_id, currency, base_date, quotes=None, quote_set=None, **params)"
    )]
    #[allow(clippy::too_many_arguments)]
    fn cap_floor_hull_white(
        py: Python<'_>,
        id: &str,
        discount_curve_id: &str,
        forward_curve_id: &str,
        currency: &Bound<'_, PyAny>,
        base_date: &Bound<'_, PyAny>,
        quotes: Option<&Bound<'_, PyAny>>,
        quote_set: Option<String>,
        params: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        let f = fields(vec![
            ("discount_curve_id", Value::String(discount_curve_id.into())),
            ("forward_curve_id", Value::String(forward_curve_id.into())),
            ("currency", Value::String(currency_code(currency)?)),
            ("base_date", Value::String(extract_date_iso(base_date)?)),
        ]);
        Self::build(py, "cap_floor_hull_white", id, quotes, quote_set, f, params)
    }

    /// SVI volatility-surface step from option vol quotes.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Step identifier (default quote-set name and surface id).
    /// base_date : datetime.date | str
    ///     Surface base date.
    /// underlying_ticker : str
    ///     Underlying identifier the quotes reference.
    /// quotes, quote_set, vol_surface_id
    ///     As in ``vol_surface``.
    /// **params
    ///     Optional wire fields: ``discount_curve_id``, ``target_expiries``,
    ///     ``target_strikes``, ``spot_override``, ``dividend_yield_override``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a field is unknown or has the wrong shape.
    #[staticmethod]
    #[pyo3(signature = (id, base_date, underlying_ticker, quotes = None, quote_set = None, vol_surface_id = None, **params))]
    #[pyo3(
        text_signature = "(id, base_date, underlying_ticker, quotes=None, quote_set=None, vol_surface_id=None, **params)"
    )]
    #[allow(clippy::too_many_arguments)]
    fn svi_surface(
        py: Python<'_>,
        id: &str,
        base_date: &Bound<'_, PyAny>,
        underlying_ticker: &str,
        quotes: Option<&Bound<'_, PyAny>>,
        quote_set: Option<String>,
        vol_surface_id: Option<String>,
        params: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        let f = fields(vec![
            (
                "vol_surface_id",
                Value::String(vol_surface_id.unwrap_or_else(|| id.into())),
            ),
            ("base_date", Value::String(extract_date_iso(base_date)?)),
            ("underlying_ticker", Value::String(underlying_ticker.into())),
        ]);
        Self::build(py, "svi_surface", id, quotes, quote_set, f, params)
    }

    /// Cross-currency basis curve step.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Step identifier (default quote-set name and curve id).
    /// currency : str | Currency
    ///     Foreign (collateral) currency of the produced discount curve.
    /// base_date : datetime.date | str
    ///     Curve base date.
    /// fx_spot : float
    ///     Spot FX rate used to translate the basis quotes.
    /// domestic_discount_id : str
    ///     Domestic discount curve the basis is measured against.
    /// quotes, quote_set, curve_id
    ///     As in ``discount``.
    /// **params
    ///     Optional wire fields: ``method``, ``interpolation``,
    ///     ``extrapolation``, ``conventions``, ``basis_spread_curve_id``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a field is unknown or has the wrong shape.
    #[staticmethod]
    #[pyo3(signature = (id, currency, base_date, fx_spot, domestic_discount_id, quotes = None, quote_set = None, curve_id = None, **params))]
    #[pyo3(
        text_signature = "(id, currency, base_date, fx_spot, domestic_discount_id, quotes=None, quote_set=None, curve_id=None, **params)"
    )]
    #[allow(clippy::too_many_arguments)]
    fn xccy_basis(
        py: Python<'_>,
        id: &str,
        currency: &Bound<'_, PyAny>,
        base_date: &Bound<'_, PyAny>,
        fx_spot: f64,
        domestic_discount_id: &str,
        quotes: Option<&Bound<'_, PyAny>>,
        quote_set: Option<String>,
        curve_id: Option<String>,
        params: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        let f = fields(vec![
            (
                "curve_id",
                Value::String(curve_id.unwrap_or_else(|| id.into())),
            ),
            ("currency", Value::String(currency_code(currency)?)),
            ("base_date", Value::String(extract_date_iso(base_date)?)),
            ("fx_spot", Value::from(fx_spot)),
            (
                "domestic_discount_id",
                Value::String(domestic_discount_id.into()),
            ),
        ]);
        Self::build(py, "xccy_basis", id, quotes, quote_set, f, params)
    }

    /// Parametric (Nelson-Siegel / Svensson) curve fit step.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Step identifier (default quote-set name and curve id).
    /// base_date : datetime.date | str
    ///     Curve base date.
    /// model : str, default "ns"
    ///     Parametric family (``"ns"`` Nelson-Siegel or ``"nss"`` Svensson).
    /// quotes, quote_set, curve_id
    ///     As in ``discount``.
    /// **params
    ///     Optional wire field: ``initial_params``. Only single-curve discount
    ///     fitting is supported. Acceptance uses the configured
    ///     discount-curve validation tolerance without a least-squares floor.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a field is unknown or has the wrong shape.
    #[staticmethod]
    #[pyo3(signature = (id, base_date, model = "ns", quotes = None, quote_set = None, curve_id = None, **params))]
    #[pyo3(
        text_signature = "(id, base_date, model='ns', quotes=None, quote_set=None, curve_id=None, **params)"
    )]
    #[allow(clippy::too_many_arguments)]
    fn parametric(
        py: Python<'_>,
        id: &str,
        base_date: &Bound<'_, PyAny>,
        model: &str,
        quotes: Option<&Bound<'_, PyAny>>,
        quote_set: Option<String>,
        curve_id: Option<String>,
        params: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        let f = fields(vec![
            (
                "curve_id",
                Value::String(curve_id.unwrap_or_else(|| id.into())),
            ),
            ("base_date", Value::String(extract_date_iso(base_date)?)),
            ("model", Value::String(model.into())),
        ]);
        Self::build(py, "parametric", id, quotes, quote_set, f, params)
    }

    /// Step identifier.
    #[getter]
    fn id(&self) -> String {
        self.inner.id.clone()
    }

    /// Name of the quote set this step reads from ``plan.quote_sets``.
    #[getter]
    fn quote_set(&self) -> String {
        self.inner.quote_set.clone()
    }

    /// Step kind (``"discount"``, ``"forward"``, ``"hazard"``, ...).
    #[getter]
    fn kind(&self) -> PyResult<String> {
        let value = serde_json::to_value(&self.inner.params)
            .map_err(|e| serde_json_to_py(e, "failed to serialize step params"))?;
        Ok(value
            .get("kind")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string())
    }

    /// Kind-specific parameters as a dict (including ``kind``).
    #[getter]
    fn params<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.params)
    }

    /// Identifiers of the quotes attached at construction.
    #[getter]
    fn quote_ids(&self) -> Vec<String> {
        self.quotes.iter().map(|q| q.id().to_string()).collect()
    }

    /// Quotes attached at construction as ``market_data`` dicts.
    #[getter]
    fn quotes<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.quotes)
    }

    /// Serialize the step (without attached quotes) to compact JSON.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If serialization fails.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|e| serde_json_to_py(e, "failed to serialize CalibrationStep"))
    }

    /// Rebuild a step from its wire JSON (``{"id", "quote_set", "kind", ...}``).
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``json`` is malformed or has unknown fields.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        serde_json::from_str(json)
            .map(Self::from_inner)
            .map_err(|e| serde_json_to_py(e, "invalid CalibrationStep JSON"))
    }

    /// Pickle support: step JSON plus attached quotes.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String, String))> {
        let restore = py.get_type::<Self>().getattr("_restore")?;
        let quotes = serde_json::to_string(&self.quotes)
            .map_err(|e| serde_json_to_py(e, "failed to serialize attached quotes"))?;
        Ok((restore, (self.to_json()?, quotes)))
    }

    /// Pickle helper: rebuild from step JSON and attached-quote JSON.
    #[staticmethod]
    fn _restore(step_json: &str, quotes_json: &str) -> PyResult<Self> {
        let mut step = Self::from_json(step_json)?;
        step.quotes = serde_json::from_str(quotes_json)
            .map_err(|e| serde_json_to_py(e, "invalid attached quote JSON"))?;
        Ok(step)
    }

    fn __repr__(&self) -> String {
        format!(
            "CalibrationStep(id={:?}, kind={:?}, quote_set={:?}, quotes={})",
            self.inner.id,
            self.kind().unwrap_or_default(),
            self.inner.quote_set,
            self.quotes.len()
        )
    }
}

/// Ordered calibration plan: steps, named quote sets, and solver settings.
///
/// Quotes attached to steps are collected into ``quote_sets`` (keyed by each
/// step's ``quote_set`` name) and carried along as ``market_data`` when the
/// plan is calibrated or wrapped in a ``CalibrationEnvelope``.
///
/// Examples
/// --------
/// >>> from finstack_quant.calibration import CalibrationPlan, CalibrationStep, RateQuote, calibrate
/// >>> quotes = [
/// ...     RateQuote.deposit("USD-DEP-3M", "USD-SOFR-OIS", "3M", 0.052),
/// ...     RateQuote.swap("USD-SWAP-2Y", "USD-SOFR-OIS", "2Y", 0.049),
/// ... ]
/// >>> plan = CalibrationPlan([CalibrationStep.discount("USD-OIS", "USD", "2026-05-08", quotes=quotes)])
/// >>> plan.quote_sets
/// {'USD-OIS': ['USD-DEP-3M', 'USD-SWAP-2Y']}
/// >>> calibrate(plan).success
/// True
#[pyclass(
    name = "CalibrationPlan",
    module = "finstack_quant.calibration",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyCalibrationPlan {
    pub(crate) inner: CalibrationPlan,
    /// Market data attached through the steps.
    pub(crate) market_data: Vec<MarketDatum>,
}

impl PyCalibrationPlan {
    pub(crate) fn from_inner(inner: CalibrationPlan) -> Self {
        Self {
            inner,
            market_data: Vec::new(),
        }
    }

    /// Wrap this plan (plus attached quotes) into a request envelope.
    pub(crate) fn to_envelope(
        &self,
        extra_market_data: Vec<MarketDatum>,
        prior_market: Vec<PriorMarketObject>,
    ) -> CalibrationEnvelope {
        let mut market_data = self.market_data.clone();
        market_data.extend(extra_market_data);
        CalibrationEnvelope::new(self.inner.clone(), market_data, prior_market)
    }
}

#[pymethods]
impl PyCalibrationPlan {
    /// Build a plan from typed steps.
    ///
    /// Parameters
    /// ----------
    /// steps : list[CalibrationStep]
    ///     Steps in execution order; attached quotes populate ``quote_sets``.
    /// id : str, default "plan"
    ///     Plan identifier.
    /// description : str | None, default None
    ///     Free-text description.
    /// settings : CalibrationConfig | dict | None, default None
    ///     Solver settings; a dict is overlaid onto the Rust defaults.
    /// quote_sets : dict[str, list[str]] | None, default None
    ///     Explicit quote sets (ids must exist in the envelope
    ///     ``market_data``); merged with the sets derived from attached quotes.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``settings`` is invalid or two steps attach quotes under the
    ///     same set name with different ids, or reuse a quote id with a
    ///     different payload. Identical attached quotes are collected once,
    ///     including when their set is supplied explicitly.
    #[new]
    #[pyo3(signature = (steps, id = "plan", description = None, settings = None, quote_sets = None))]
    #[pyo3(text_signature = "(steps, id='plan', description=None, settings=None, quote_sets=None)")]
    fn new(
        py: Python<'_>,
        steps: Vec<PyRef<'_, PyCalibrationStep>>,
        id: &str,
        description: Option<String>,
        settings: Option<&Bound<'_, PyAny>>,
        quote_sets: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        let settings = extract_config(py, settings)?;
        let mut sets: IndexMap<String, Vec<finstack_quant_calibration::quotes::ids::QuoteId>> =
            IndexMap::new();
        if let Some(quote_sets) = quote_sets {
            for (name, ids) in quote_sets.iter() {
                let name: String = name.extract()?;
                let ids: Vec<String> = ids.extract()?;
                sets.insert(
                    name,
                    ids.into_iter()
                        .map(finstack_quant_calibration::quotes::ids::QuoteId::new)
                        .collect(),
                );
            }
        }
        let mut market_data = Vec::new();
        let mut payloads = IndexMap::new();
        let mut inner_steps = Vec::with_capacity(steps.len());
        for step in steps {
            if !step.quotes.is_empty() {
                let ids: Vec<_> = step
                    .quotes
                    .iter()
                    .map(|q| finstack_quant_calibration::quotes::ids::QuoteId::new(q.id()))
                    .collect();
                match sets.get(&step.inner.quote_set) {
                    Some(existing) if existing != &ids => {
                        return Err(value_error(format!(
                            "quote set '{}' is attached by more than one step with different quotes",
                            step.inner.quote_set
                        )));
                    }
                    Some(_) => {}
                    None => {
                        sets.insert(step.inner.quote_set.clone(), ids);
                    }
                }
                for quote in &step.quotes {
                    let payload = serde_json::to_value(quote)
                        .map_err(|error| value_error(error.to_string()))?;
                    match payloads.get(quote.id()) {
                        Some(existing) if existing != &payload => {
                            return Err(value_error(format!(
                                "quote id '{}' has conflicting attached payloads",
                                quote.id()
                            )));
                        }
                        Some(_) => {}
                        None => {
                            payloads.insert(quote.id().to_string(), payload);
                            market_data.push(quote.clone());
                        }
                    }
                }
            }
            inner_steps.push(step.inner.clone());
        }
        Ok(Self {
            inner: CalibrationPlan {
                id: id.to_string(),
                description,
                quote_sets: sets,
                steps: inner_steps,
                settings,
            },
            market_data,
        })
    }

    /// Plan identifier.
    #[getter]
    fn id(&self) -> String {
        self.inner.id.clone()
    }

    /// Plan description, when set.
    #[getter]
    fn description(&self) -> Option<String> {
        self.inner.description.clone()
    }

    /// Step identifiers in execution order.
    #[getter]
    fn step_ids(&self) -> Vec<String> {
        self.inner.steps.iter().map(|s| s.id.clone()).collect()
    }

    /// Typed steps in execution order (without attached quotes).
    #[getter]
    fn steps(&self) -> Vec<PyCalibrationStep> {
        self.inner
            .steps
            .iter()
            .cloned()
            .map(PyCalibrationStep::from_inner)
            .collect()
    }

    /// Named quote sets: ``{name: [quote_id, ...]}``.
    #[getter]
    fn quote_sets<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let dict = PyDict::new(py);
        for (name, ids) in &self.inner.quote_sets {
            let list: Vec<&str> = ids.iter().map(|id| id.as_str()).collect();
            dict.set_item(name, PyList::new(py, list)?)?;
        }
        Ok(dict)
    }

    /// Solver settings.
    #[getter]
    fn settings(&self) -> super::config::PyCalibrationConfig {
        super::config::PyCalibrationConfig::from_inner(self.inner.settings.clone())
    }

    /// Market data attached through the steps, as ``market_data`` dicts.
    #[getter]
    fn market_data<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.market_data)
    }

    /// Serialize the plan (without attached market data) to compact JSON.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If serialization fails.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|e| serde_json_to_py(e, "failed to serialize CalibrationPlan"))
    }

    /// Rebuild a plan from its wire JSON.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``json`` is malformed or has unknown fields.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        serde_json::from_str(json)
            .map(Self::from_inner)
            .map_err(|e| serde_json_to_py(e, "invalid CalibrationPlan JSON"))
    }

    /// Pickle support: plan JSON plus attached market data.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String, String))> {
        let restore = py.get_type::<Self>().getattr("_restore")?;
        let market_data = serde_json::to_string(&self.market_data)
            .map_err(|e| serde_json_to_py(e, "failed to serialize attached market data"))?;
        Ok((restore, (self.to_json()?, market_data)))
    }

    /// Pickle helper: rebuild from plan JSON and attached market-data JSON.
    #[staticmethod]
    fn _restore(plan_json: &str, market_data_json: &str) -> PyResult<Self> {
        let mut plan = Self::from_json(plan_json)?;
        plan.market_data = serde_json::from_str(market_data_json)
            .map_err(|e| serde_json_to_py(e, "invalid attached market data JSON"))?;
        Ok(plan)
    }

    fn __repr__(&self) -> String {
        format!(
            "CalibrationPlan(id={:?}, steps={:?}, quote_sets={})",
            self.inner.id,
            self.step_ids(),
            self.inner.quote_sets.len()
        )
    }
}

/// Complete calibration request: plan, flat market data, and prior calibrated objects.
///
/// Examples
/// --------
/// >>> from finstack_quant.calibration import CalibrationEnvelope, CalibrationPlan
/// >>> envelope = CalibrationEnvelope(CalibrationPlan([], id="smoke"))
/// >>> envelope.schema
/// 'finstack_quant.calibration/1'
/// >>> len(envelope.content_hash()) > 0
/// True
#[pyclass(
    name = "CalibrationEnvelope",
    module = "finstack_quant.calibration",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyCalibrationEnvelope {
    pub(crate) inner: CalibrationEnvelope,
}

impl PyCalibrationEnvelope {
    pub(crate) fn from_inner(inner: CalibrationEnvelope) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyCalibrationEnvelope {
    /// Assemble a request envelope.
    ///
    /// Parameters
    /// ----------
    /// plan : CalibrationPlan
    ///     Plan to execute; quotes attached to its steps are included.
    /// market_data : list[RateQuote | CdsQuote | VolQuote | dict] | None, default None
    ///     Additional flat market data (typed quotes or ``{"kind": ...}`` dicts
    ///     such as ``fx_spot``, ``price``, ``fixing_series``).
    /// prior_market : list[dict] | None, default None
    ///     Pre-built calibrated objects as ``{"kind": ..., ...}`` dicts.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a market-data or prior-market entry has an invalid shape.
    #[new]
    #[pyo3(signature = (plan, market_data = None, prior_market = None))]
    #[pyo3(text_signature = "(plan, market_data=None, prior_market=None)")]
    fn new(
        py: Python<'_>,
        plan: PyRef<'_, PyCalibrationPlan>,
        market_data: Option<&Bound<'_, PyAny>>,
        prior_market: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        let market_data = extract_market_data(py, market_data)?;
        let prior_market = extract_prior_market(py, prior_market)?;
        Ok(Self::from_inner(
            plan.to_envelope(market_data, prior_market),
        ))
    }

    /// Schema marker (``"finstack_quant.calibration/1"``).
    #[getter]
    fn schema(&self) -> PyResult<String> {
        let value = serde_json::to_value(self.inner.schema)
            .map_err(|e| serde_json_to_py(e, "failed to serialize schema marker"))?;
        Ok(value.as_str().unwrap_or_default().to_string())
    }

    /// The calibration plan.
    #[getter]
    fn plan(&self) -> PyCalibrationPlan {
        PyCalibrationPlan::from_inner(self.inner.plan.clone())
    }

    /// Flat market data as ``{"kind": ..., ...}`` dicts.
    #[getter]
    fn market_data<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.market_data)
    }

    /// Prior calibrated objects as ``{"kind": ..., ...}`` dicts.
    #[getter]
    fn prior_market<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.prior_market)
    }

    /// Versioned SHA-256 hash of the canonical request JSON.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the request contains a non-finite number.
    fn content_hash(&self) -> PyResult<String> {
        self.inner.content_hash().map_err(core_to_py)
    }

    /// Solver-free static validation of this envelope.
    ///
    /// Returns
    /// -------
    /// CalibrationValidationReport
    ///     Every static error plus the dependency graph.
    fn dry_run(&self) -> PyCalibrationValidationReport {
        PyCalibrationValidationReport::from_inner(validate_api::validate(&self.inner))
    }

    /// Serialize to compact JSON.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If serialization fails.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|e| serde_json_to_py(e, "failed to serialize CalibrationEnvelope"))
    }

    /// Strictly load an envelope from JSON.
    ///
    /// Raises
    /// ------
    /// CalibrationEnvelopeError
    ///     If the JSON is malformed, the schema marker is missing or
    ///     unsupported, or the structure is invalid; ``diagnostics`` lists the
    ///     pointer-level findings.
    #[staticmethod]
    fn from_json(py: Python<'_>, json: &str) -> PyResult<Self> {
        super::parse_envelope_json(py, json).map(Self::from_inner)
    }

    /// Pickle support through the JSON wire format.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        format!(
            "CalibrationEnvelope(plan={:?}, steps={}, market_data={}, prior_market={})",
            self.inner.plan.id,
            self.inner.plan.steps.len(),
            self.inner.market_data.len(),
            self.inner.prior_market.len()
        )
    }
}
