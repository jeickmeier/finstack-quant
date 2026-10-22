//! Typed calibration market data: rate, CDS, volatility, inflation,
//! cross-currency and CDS-tranche quotes plus the FX-spot, price,
//! dividend-schedule and collateral datums carried by the calibration envelope.
//!
//! Quote constructors convert host arguments and delegate to the typed Rust
//! quote constructors, which own defaults and quote validation. The datum
//! constructors convert host values into the Rust payload structs.

use super::envelope::{currency_code, from_value};
use crate::bindings::core::currency::extract_currency;
use crate::bindings::core::market_data::scalars::extract_f64;
use crate::bindings::core::money::{decimal_to_py, money_from_amount};
use crate::bindings::date_utils::{date_to_py, py_to_date};
use crate::bindings::extract::{extract_basis_points, extract_rate_decimal};
use crate::bindings::macros::wire_methods;
use crate::bindings::module_utils::{py_to_json_value, py_to_serde};
use crate::bindings::pandas_utils::serde_to_py;
use crate::bindings::repr_support::repr_from_serde;
use crate::errors::core_to_py;
use finstack_quant_calibration::api::market_datum::{
    DividendScheduleDatum, FxSpotDatum, MarketDatum, PriceDatum,
};
use finstack_quant_calibration::quotes::CdsQuote;
use finstack_quant_calibration::quotes::CdsTrancheQuote;
use finstack_quant_calibration::quotes::InflationQuote;
use finstack_quant_calibration::quotes::Pillar;
use finstack_quant_calibration::quotes::RateQuote;
use finstack_quant_calibration::quotes::VolQuote;
use finstack_quant_calibration::quotes::XccyQuote;
use finstack_quant_core::market_data::scalars::MarketScalar;
use pyo3::prelude::*;
use pyo3::IntoPyObjectExt;

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
        CdsQuote::par_spread(
            id,
            entity,
            &currency_code(currency)?,
            doc_clause,
            extract_pillar(py, pillar)?,
            extract_basis_points(spread_bp, "spread_bp")?,
            recovery_rate,
        )
        .map(Self::from_inner)
        .map_err(core_to_py)
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
    /// coupon_bp : float | Bps
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
        text_signature = "(id, entity, currency, doc_clause, pillar, coupon_bp, upfront_pct, recovery_rate)"
    )]
    #[allow(clippy::too_many_arguments)]
    fn upfront(
        py: Python<'_>,
        id: &str,
        entity: &str,
        currency: &Bound<'_, PyAny>,
        doc_clause: &str,
        pillar: &Bound<'_, PyAny>,
        coupon_bp: &Bound<'_, PyAny>,
        upfront_pct: f64,
        recovery_rate: f64,
    ) -> PyResult<Self> {
        CdsQuote::upfront(
            id,
            entity,
            &currency_code(currency)?,
            doc_clause,
            extract_pillar(py, pillar)?,
            extract_basis_points(coupon_bp, "coupon_bp")?,
            upfront_pct,
            recovery_rate,
        )
        .map(Self::from_inner)
        .map_err(core_to_py)
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
    #[allow(clippy::too_many_arguments)]
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
        CdsTrancheQuote::new(
            id,
            index,
            series,
            attachment,
            detachment,
            py_to_date(maturity)?,
            upfront_pct,
            extract_basis_points(coupon_bp, "coupon_bp")?,
            &currency_code(currency)?,
            doc_clause,
        )
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
    ///     ``Decimal`` converts to ``float`` (Rust ``decimal_to_f64``).
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

    fn __repr__(&self) -> String {
        repr_from_serde("PriceDatum", &self.inner)
    }
}

wire_methods!(PyPriceDatum, PriceDatum, "PriceDatum");

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

    fn __repr__(&self) -> String {
        repr_from_serde("DividendScheduleDatum", &self.inner)
    }
}

wire_methods!(
    PyDividendScheduleDatum,
    DividendScheduleDatum,
    "DividendScheduleDatum"
);

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
    if let Ok(datum) = obj.cast::<PyDividendScheduleDatum>() {
        return Ok(MarketDatum::DividendSchedule(datum.borrow().inner.clone()));
    }
    let value = py_to_json_value(py, obj, "market datum")?;
    from_value(value, "market datum (expected {\"kind\": ..., ...})")
}
