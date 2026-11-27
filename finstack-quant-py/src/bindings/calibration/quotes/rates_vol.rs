//! Rates, CDS, and volatility calibration quote Python bindings.

use super::super::envelope::from_value;
use crate::bindings::core::currency::extract_currency;
use crate::bindings::date_utils::py_to_date;
use crate::bindings::extract::{extract_basis_points, extract_rate_decimal};
use crate::bindings::json_bridge::serde_to_py;
use crate::bindings::macros::wire_methods;
use crate::bindings::module_utils::py_to_serde;
use crate::bindings::pydoc::pydoc;
use crate::bindings::repr_support::repr_from_serde;
use crate::errors::core_to_py;
use finstack_quant_calibration::quotes::CdsQuote;
use finstack_quant_calibration::quotes::Pillar;
use finstack_quant_calibration::quotes::RateQuote;
use finstack_quant_calibration::quotes::VolQuote;
use pyo3::prelude::*;
use serde_json::{Map, Value};

/// Pillar from a tenor / ISO-date string, ``datetime.date``, or wire mapping.
pub(crate) fn extract_pillar(py: Python<'_>, obj: &Bound<'_, PyAny>) -> PyResult<Pillar> {
    if let Ok(text) = obj.extract::<String>() {
        return text.parse::<Pillar>().map_err(core_to_py);
    }
    if let Ok(date) = py_to_date(obj) {
        return Ok(Pillar::Date(date));
    }
    py_to_serde(py, obj, "pillar")
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
        RateQuote::deposit(
            id,
            index,
            extract_pillar(py, pillar)?,
            extract_rate_decimal(rate, "rate")?,
        )
        .map(Self::from_inner)
        .map_err(core_to_py)
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
        RateQuote::fra(
            id,
            index,
            extract_pillar(py, start)?,
            extract_pillar(py, end)?,
            extract_rate_decimal(rate, "rate")?,
        )
        .map(Self::from_inner)
        .map_err(core_to_py)
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
    /// convexity_adjustment : float, optional
    ///     Convexity adjustment as a decimal rate subtracted from the
    ///     futures-implied forward (Hull convention). ``None`` uses the Rust
    ///     authoring default ``RateQuote::DEFAULT_CONVEXITY_ADJUSTMENT``
    ///     (``0.0``, no adjustment).
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the date cannot be parsed or the price is not finite.
    #[staticmethod]
    #[pyo3(signature = (id, contract, expiry, price, convexity_adjustment = None))]
    #[pyo3(text_signature = "(id, contract, expiry, price, convexity_adjustment=None)")]
    fn futures(
        id: &str,
        contract: &str,
        expiry: &Bound<'_, PyAny>,
        price: f64,
        convexity_adjustment: Option<f64>,
    ) -> PyResult<Self> {
        RateQuote::futures(
            id,
            contract,
            py_to_date(expiry)?,
            price,
            convexity_adjustment,
        )
        .map(Self::from_inner)
        .map_err(core_to_py)
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
    ///     Optional finite floating-leg spread as a decimal (``0.0010`` = 10 bp).
    ///     ``None`` omits the spread; non-finite numbers are rejected.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the pillar cannot be parsed or the rate or supplied spread is not finite.
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
        RateQuote::swap(
            id,
            index,
            extract_pillar(py, pillar)?,
            extract_rate_decimal(rate, "rate")?,
            spread_decimal,
        )
        .map(Self::from_inner)
        .map_err(core_to_py)
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

    fn __repr__(&self) -> String {
        repr_from_serde("RateQuote", &self.inner)
    }
}

wire_methods!(PyRateQuote, RateQuote, "RateQuote", validate);

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
}

#[pymethods]
impl PyCdsQuote {
    #[doc = pydoc!("finstack_quant.calibration._quotes.rates_vol", "CdsQuote.par_spread")]
    #[staticmethod]
    #[pyo3(text_signature = "(id, entity, currency, doc_clause, pillar, spread_bp, recovery_rate)")]
    fn par_spread(
        id: &str,
        entity: &str,
        currency: &Bound<'_, PyAny>,
        doc_clause: &str,
        pillar: &Bound<'_, PyAny>,
        spread_bp: &Bound<'_, PyAny>,
        recovery_rate: f64,
    ) -> PyResult<Self> {
        let py = currency.py();
        CdsQuote::par_spread(
            id,
            entity,
            extract_currency(currency)?.as_ref(),
            doc_clause,
            extract_pillar(py, pillar)?,
            extract_basis_points(spread_bp, "spread_bp")?,
            recovery_rate,
        )
        .map(Self::from_inner)
        .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.calibration._quotes.rates_vol", "CdsQuote.upfront")]
    #[staticmethod]
    #[pyo3(text_signature = "(params)")]
    fn upfront(params: &Bound<'_, PyAny>) -> PyResult<Self> {
        let mut fields: Map<String, Value> =
            py_to_serde(params.py(), params, "CdsQuote.upfront params")?;
        fields.insert("type".to_string(), Value::from("cds_upfront"));
        let quote: CdsQuote = from_value(Value::Object(fields), "CdsQuote.upfront params")?;
        quote.validate().map_err(core_to_py)?;
        Ok(Self::from_inner(quote))
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

    /// Reference entity name.
    #[getter]
    fn entity(&self) -> String {
        self.inner.entity().to_string()
    }

    /// CDS convention as ``{"currency": ..., "doc_clause": ...}``.
    #[getter]
    fn convention<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, self.inner.convention())
    }

    /// Maturity pillar as its wire mapping (``{"tenor": ...}`` or ``{"date": ...}``).
    #[getter]
    fn pillar<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, self.inner.pillar())
    }

    /// Assumed recovery rate as a decimal.
    #[getter]
    fn recovery_rate(&self) -> f64 {
        self.inner.recovery_rate()
    }

    /// Running coupon, in basis points, of the CDS built from this quote:
    /// the par spread for ``cds_par_spread`` quotes and the contractual
    /// running coupon for ``cds_upfront`` quotes.
    #[getter]
    fn coupon_bp(&self) -> f64 {
        self.inner.coupon_bp()
    }

    fn __repr__(&self) -> String {
        repr_from_serde("CdsQuote", &self.inner)
    }
}

wire_methods!(PyCdsQuote, CdsQuote, "CdsQuote", validate);

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
    /// option_type : str, optional
    ///     ``"call"`` or ``"put"``. ``None`` uses the Rust authoring default
    ///     ``VolQuote::DEFAULT_OPTION_TYPE`` (``"call"``).
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the date or numeric inputs are invalid.
    #[staticmethod]
    #[pyo3(signature = (id, underlying, expiry, strike, vol, option_type = None))]
    #[pyo3(text_signature = "(id, underlying, expiry, strike, vol, option_type=None)")]
    fn option_vol(
        id: &str,
        underlying: &str,
        expiry: &Bound<'_, PyAny>,
        strike: f64,
        vol: f64,
        option_type: Option<&str>,
    ) -> PyResult<Self> {
        VolQuote::option_vol(
            id,
            underlying,
            py_to_date(expiry)?,
            strike,
            vol,
            option_type,
        )
        .map(Self::from_inner)
        .map_err(core_to_py)
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
    /// quote_type : str
    ///     ``"normal"`` (``vol`` is an absolute rate volatility) or
    ///     ``"black_lognormal"`` (``vol`` is a relative volatility). Required:
    ///     it decides the unit of ``vol``, so there is no default.
    /// convention : str, optional
    ///     Swaption market convention identifier. ``None`` uses the Rust
    ///     authoring default ``VolQuote::DEFAULT_SWAPTION_CONVENTION``
    ///     (``"USD"``).
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a date or numeric input is invalid or ``quote_type`` is unknown.
    #[staticmethod]
    #[pyo3(signature = (id, expiry, maturity, strike, vol, quote_type, convention = None))]
    #[pyo3(text_signature = "(id, expiry, maturity, strike, vol, quote_type, convention=None)")]
    fn swaption_vol(
        id: &str,
        expiry: &Bound<'_, PyAny>,
        maturity: &Bound<'_, PyAny>,
        strike: f64,
        vol: f64,
        quote_type: &str,
        convention: Option<&str>,
    ) -> PyResult<Self> {
        VolQuote::swaption_vol(
            id,
            py_to_date(expiry)?,
            py_to_date(maturity)?,
            strike,
            vol,
            quote_type,
            convention,
        )
        .map(Self::from_inner)
        .map_err(core_to_py)
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
    /// quote_type : str
    ///     ``"normal"`` (``vol`` is an absolute rate volatility) or
    ///     ``"black_lognormal"`` (``vol`` is a relative volatility). Required:
    ///     it decides the unit of ``vol``, so there is no default.
    /// is_cap : bool, optional
    ///     ``True`` for a cap, ``False`` for a floor. ``None`` uses the Rust
    ///     authoring default ``VolQuote::DEFAULT_IS_CAP`` (a cap).
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the date or numeric inputs are invalid or ``quote_type`` is unknown.
    #[staticmethod]
    #[pyo3(signature = (id, expiry, strike, vol, quote_type, is_cap = None))]
    #[pyo3(text_signature = "(id, expiry, strike, vol, quote_type, is_cap=None)")]
    fn cap_floor_vol(
        id: &str,
        expiry: &Bound<'_, PyAny>,
        strike: f64,
        vol: f64,
        quote_type: &str,
        is_cap: Option<bool>,
    ) -> PyResult<Self> {
        VolQuote::cap_floor_vol(id, py_to_date(expiry)?, strike, vol, quote_type, is_cap)
            .map(Self::from_inner)
            .map_err(core_to_py)
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

    fn __repr__(&self) -> String {
        repr_from_serde("VolQuote", &self.inner)
    }
}

wire_methods!(PyVolQuote, VolQuote, "VolQuote", validate);
