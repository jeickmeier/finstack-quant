//! Typed equity instruments: `EquityOption`.
//! Mirrors the `PyInterestRateSwap` pattern in `typed_rates.rs`.

use pyo3::prelude::*;
use pyo3::types::PyDict;

use crate::bindings::core::money::PyMoney;
use crate::bindings::date_utils::{date_to_py, extract_date};
use crate::bindings::extract::extract_market;
use crate::errors::core_to_py;
use finstack_quant_core::types::{CurveId, InstrumentId, PriceId};
use finstack_quant_valuations::instruments::equity::equity_option::EquityOptionMarketData;
use finstack_quant_valuations::instruments::InstrumentJson;

use super::convert::{
    attributes_from_py, bool_repr, builder_repr, date_repr, day_count_from_py, enum_to_py_string,
    float_repr, money_from_py, money_repr, money_to_py,
};
use super::instruments::{enum_from_str, serialize_typed_instrument_json};
use super::typed_fx::{
    envelope_metric_value, envelope_option_greeks, instrument_envelope_methods,
    instrument_pricing_methods, take_builder,
};

type EquityOptionBuilderInner =
    finstack_quant_valuations::instruments::equity::equity_option::EquityOptionBuilder;

/// Vanilla equity option (typed wrapper for Rust ``EquityOption``).
///
/// European options price with Black–Scholes–Merton (``"black76"`` on the
/// forward); American and Bermudan styles use the tree pricer. The
/// ``notional`` scales the per-share value (contract size in currency
/// units); discrete dividends and a continuous ``div_yield_id`` are both
/// supported.
///
/// Build with ``EquityOption.builder()`` or ``EquityOption.european_call(...)``;
/// start from ``EquityOption.example()``. Instances are accepted directly by
/// ``price_instrument`` and expose ``price`` / ``metric`` / ``greeks`` /
/// ``implied_vol`` themselves.
///
/// Examples
/// --------
/// >>> from finstack_quant.valuations.instruments import EquityOption
/// >>> opt = EquityOption.example()
/// >>> (opt.underlying_ticker, opt.strike, opt.option_type, opt.exercise_style)
/// ('SPX', 4500.0, 'call', 'european')
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "EquityOption",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyEquityOption {
    /// Inner canonical Rust equity option.
    pub(crate) inner: finstack_quant_valuations::instruments::EquityOption,
}

impl PyEquityOption {
    /// Serialize as the canonical instrument envelope accepted by the JSON loader.
    pub(crate) fn envelope_json(&self) -> PyResult<String> {
        serialize_typed_instrument_json(
            InstrumentJson::EquityOption(self.inner.clone()),
            "EquityOption",
        )
    }
}

instrument_envelope_methods!(
    PyEquityOption,
    EquityOption,
    "equity_option",
    PyEquityOptionBuilder,
    finstack_quant_valuations::instruments::EquityOption::builder()
);
instrument_pricing_methods!(PyEquityOption);

#[pymethods]
impl PyEquityOption {
    /// Canonical example: SPX 4500 European call expiring 2024-06-21.
    ///
    /// Mirrors Rust ``EquityOption::example()``: USD 100 notional, curve
    /// ``USD-OIS``, spot ``EQUITY-SPOT``, surface ``EQUITY-VOL``, dividend
    /// yield ``EQUITY-DIVYIELD``.
    ///
    /// Returns
    /// -------
    /// EquityOption
    ///     The validated example option.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the canonical example fails validation (never for a released build).
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn example() -> PyResult<Self> {
        finstack_quant_valuations::instruments::EquityOption::example()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    /// Build a cash-settled European call.
    ///
    /// Mirrors Rust ``EquityOption::european_call`` /
    /// ``european_call_with_market_data``: the market-data identifiers default
    /// to the same generic ids the Rust constructor uses (``"USD-OIS"``,
    /// ``"EQUITY-SPOT"``, ``"EQUITY-VOL"``, ``"EQUITY-DIVYIELD"``); pass your
    /// own to bind the option to real market objects.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Unique instrument identifier.
    /// ticker : str
    ///     Underlying equity ticker.
    /// strike : float
    ///     Strike price; must be finite and positive.
    /// expiry : datetime.date | str
    ///     Expiry date.
    /// notional : Money | float
    ///     Notional for valuation scaling; a bare float is USD.
    /// discount_curve_id : str
    ///     Discount curve identifier.
    /// spot_id : str
    ///     Equity spot price identifier.
    /// vol_surface_id : str
    ///     Volatility surface identifier.
    /// div_yield_id : str | None
    ///     Continuous dividend yield identifier; ``None`` for no yield.
    ///
    /// Returns
    /// -------
    /// EquityOption
    ///     The validated option.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``strike`` is not positive or the notional is zero.
    #[staticmethod]
    #[pyo3(signature = (id, ticker, strike, expiry, notional, *, discount_curve_id="USD-OIS",
                        spot_id="EQUITY-SPOT", vol_surface_id="EQUITY-VOL",
                        div_yield_id="EQUITY-DIVYIELD"))]
    #[pyo3(
        text_signature = "(id, ticker, strike, expiry, notional, *, discount_curve_id='USD-OIS', \
spot_id='EQUITY-SPOT', vol_surface_id='EQUITY-VOL', div_yield_id='EQUITY-DIVYIELD')"
    )]
    // PyO3 binding: the argument list mirrors the Python keyword-argument API.
    #[allow(clippy::too_many_arguments)]
    fn european_call(
        id: &str,
        ticker: &str,
        strike: f64,
        expiry: &Bound<'_, PyAny>,
        notional: &Bound<'_, PyAny>,
        discount_curve_id: &str,
        spot_id: &str,
        vol_surface_id: &str,
        div_yield_id: Option<&str>,
    ) -> PyResult<Self> {
        let mut market_data =
            EquityOptionMarketData::new(discount_curve_id, spot_id, vol_surface_id);
        if let Some(div) = div_yield_id {
            market_data = market_data.with_dividend_yield(div);
        }
        let inner =
            finstack_quant_valuations::instruments::EquityOption::european_call_with_market_data(
                id,
                ticker,
                strike,
                extract_date(expiry)?,
                money_from_py(notional, Some("USD"), "notional")?,
                market_data,
            )
            .map_err(core_to_py)?;
        Ok(Self { inner })
    }

    /// Implied volatility that reproduces ``market_price``.
    ///
    /// Uses the configured exercise engine, dividend schedule and model day count;
    /// trial volatility replaces the active surface or override.
    ///
    /// Parameters
    /// ----------
    /// market : MarketContext | str
    ///     Market carrying the discount curve, spot and (optional) dividend yield.
    /// as_of : datetime.date | str
    ///     Valuation date strictly before expiry and observed exercise.
    /// market_price : float
    ///     Finite non-negative total trade PV in the notional currency.
    ///
    /// Returns
    /// -------
    /// float
    ///     Annualized lognormal volatility as a decimal (``0.20`` = 20%).
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If PV or notional is invalid, exercise has occurred, or volatility is unidentifiable.
    /// KeyError
    ///     If required market data is missing from ``market``.
    /// RuntimeError
    ///     If the root search does not converge.
    #[pyo3(text_signature = "($self, market, as_of, market_price)")]
    fn implied_vol(
        &self,
        py: Python<'_>,
        market: &Bound<'_, PyAny>,
        as_of: &Bound<'_, PyAny>,
        market_price: f64,
    ) -> PyResult<f64> {
        let market = extract_market(py, market)?;
        let as_of = extract_date(as_of)?;
        self.inner
            .implied_vol(&market, as_of, market_price)
            .map_err(core_to_py)
    }

    /// Spot delta of the option.
    ///
    /// Returns
    /// -------
    /// float
    ///     Delta produced by the selected model.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If an input is invalid, required market data is missing, pricing
    ///     fails, or the model does not produce delta.
    #[pyo3(signature = (market, as_of, model="default"))]
    fn delta(
        &self,
        py: Python<'_>,
        market: &Bound<'_, PyAny>,
        as_of: &Bound<'_, PyAny>,
        model: &str,
    ) -> PyResult<f64> {
        envelope_metric_value(py, self.envelope_json()?, market, as_of, model, "delta")
    }

    /// Gamma of the option.
    ///
    /// Returns
    /// -------
    /// float
    ///     Gamma produced by the selected model.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If an input is invalid, required market data is missing, pricing
    ///     fails, or the model does not produce gamma.
    #[pyo3(signature = (market, as_of, model="default"))]
    fn gamma(
        &self,
        py: Python<'_>,
        market: &Bound<'_, PyAny>,
        as_of: &Bound<'_, PyAny>,
        model: &str,
    ) -> PyResult<f64> {
        envelope_metric_value(py, self.envelope_json()?, market, as_of, model, "gamma")
    }

    /// Vega of the option (per 1% vol).
    ///
    /// Returns
    /// -------
    /// float
    ///     Vega produced by the selected model.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If an input is invalid, required market data is missing, pricing
    ///     fails, or the model does not produce vega.
    #[pyo3(signature = (market, as_of, model="default"))]
    fn vega(
        &self,
        py: Python<'_>,
        market: &Bound<'_, PyAny>,
        as_of: &Bound<'_, PyAny>,
        model: &str,
    ) -> PyResult<f64> {
        envelope_metric_value(py, self.envelope_json()?, market, as_of, model, "vega")
    }

    /// Theta of the option (per day on ``theta_day_basis``).
    ///
    /// Returns
    /// -------
    /// float
    ///     Theta produced by the selected model.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If an input is invalid, required market data is missing, pricing
    ///     fails, or the model does not produce theta.
    #[pyo3(signature = (market, as_of, model="default"))]
    fn theta(
        &self,
        py: Python<'_>,
        market: &Bound<'_, PyAny>,
        as_of: &Bound<'_, PyAny>,
        model: &str,
    ) -> PyResult<f64> {
        envelope_metric_value(py, self.envelope_json()?, market, as_of, model, "theta")
    }

    /// Rho of the option.
    ///
    /// Returns
    /// -------
    /// float
    ///     Rho produced by the selected model.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If an input is invalid, required market data is missing, pricing
    ///     fails, or the model does not produce rho.
    #[pyo3(signature = (market, as_of, model="default"))]
    fn rho(
        &self,
        py: Python<'_>,
        market: &Bound<'_, PyAny>,
        as_of: &Bound<'_, PyAny>,
        model: &str,
    ) -> PyResult<f64> {
        envelope_metric_value(py, self.envelope_json()?, market, as_of, model, "rho")
    }

    /// Compute the standard option Greek set as a dict.
    ///
    /// Mirrors ``FxOption.greeks`` and the WASM ``greeks`` method: Greeks the
    /// selected model cannot produce are omitted, and any non-finite Greek
    /// raises rather than being returned.
    ///
    /// Returns
    /// -------
    /// dict[str, float]
    ///     Mapping of Greek name to value for every Greek the model produced.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If an input is invalid, required market data is missing, pricing
    ///     fails, or a returned Greek is non-finite.
    #[pyo3(signature = (market, as_of, model="default"))]
    fn greeks<'py>(
        &self,
        py: Python<'py>,
        market: &Bound<'py, PyAny>,
        as_of: &Bound<'py, PyAny>,
        model: &str,
    ) -> PyResult<Bound<'py, PyDict>> {
        envelope_option_greeks(py, self.envelope_json()?, market, as_of, model)
    }

    /// Underlying equity ticker.
    #[getter]
    fn underlying_ticker(&self) -> String {
        self.inner.underlying_ticker.clone()
    }

    /// Strike price.
    #[getter]
    fn strike(&self) -> f64 {
        self.inner.strike
    }

    /// ``"call"`` or ``"put"``.
    #[getter]
    fn option_type(&self) -> PyResult<String> {
        enum_to_py_string(&self.inner.option_type)
    }

    /// ``"european"``, ``"american"`` or ``"bermudan"``.
    #[getter]
    fn exercise_style(&self) -> PyResult<String> {
        enum_to_py_string(&self.inner.exercise_style)
    }

    /// Expiry date.
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.expiry)
    }

    /// Notional for valuation scaling.
    #[getter]
    fn notional(&self) -> PyMoney {
        money_to_py(self.inner.notional)
    }

    /// Day count for the time-to-expiry year fraction (serde name).
    #[getter]
    fn day_count(&self) -> PyResult<String> {
        enum_to_py_string(&self.inner.day_count)
    }

    /// Per-day theta basis: ``"calendar_365"`` or ``"trading_252"``.
    #[getter]
    fn theta_day_basis(&self) -> PyResult<String> {
        enum_to_py_string(&self.inner.theta_day_basis)
    }

    /// ``"physical"`` or ``"cash"`` settlement.
    #[getter]
    fn settlement(&self) -> PyResult<String> {
        enum_to_py_string(&self.inner.settlement)
    }

    /// Observed exercise state as ``{"date", "spot", "settlement_date", "exercised"}``, or ``None``.
    #[getter]
    fn exercise<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyDict>>> {
        let Some(exercise) = self.inner.exercise else {
            return Ok(None);
        };
        let out = PyDict::new(py);
        out.set_item("date", date_to_py(py, exercise.date)?)?;
        out.set_item("spot", exercise.spot)?;
        out.set_item("settlement_date", date_to_py(py, exercise.settlement_date)?)?;
        out.set_item("exercised", exercise.exercised)?;
        Ok(Some(out))
    }

    /// Discount curve identifier.
    #[getter]
    fn discount_curve_id(&self) -> String {
        self.inner.discount_curve_id.to_string()
    }

    /// Equity spot price identifier.
    #[getter]
    fn spot_id(&self) -> String {
        self.inner.spot_id.to_string()
    }

    /// Volatility surface identifier.
    #[getter]
    fn vol_surface_id(&self) -> String {
        self.inner.vol_surface_id.to_string()
    }

    /// Continuous dividend yield identifier, or ``None``.
    #[getter]
    fn div_yield_id(&self) -> Option<String> {
        self.inner.div_yield_id.as_ref().map(|id| id.to_string())
    }

    /// Discrete dividends as ``(ex_date, amount)`` pairs.
    #[getter]
    fn discrete_dividends<'py>(&self, py: Python<'py>) -> PyResult<Vec<(Bound<'py, PyAny>, f64)>> {
        self.inner
            .discrete_dividends
            .iter()
            .map(|(date, amount)| Ok((date_to_py(py, *date)?, *amount)))
            .collect()
    }

    /// Bermudan exercise dates, or ``None``.
    #[getter]
    fn exercise_schedule<'py>(&self, py: Python<'py>) -> PyResult<Option<Vec<Bound<'py, PyAny>>>> {
        self.inner
            .exercise_schedule
            .as_ref()
            .map(|dates| dates.iter().map(|d| date_to_py(py, *d)).collect())
            .transpose()
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "EquityOption(id={:?}, underlying_ticker={:?}, option_type={:?}, strike={}, expiry={}, exercise_style={:?}, notional={})",
            self.inner.id.as_str(),
            self.inner.underlying_ticker,
            enum_to_py_string(&self.inner.option_type).unwrap_or_default(),
            float_repr(self.inner.strike),
            date_repr(self.inner.expiry),
            enum_to_py_string(&self.inner.exercise_style).unwrap_or_default(),
            money_repr(self.inner.notional),
        )
    }
}

/// Fluent builder for ``EquityOption``; wraps the Rust
/// ``FinancialBuilder``-generated builder (consuming setters).
///
/// Builders are consumed by ``build()``; create a new builder per instrument.
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "EquityOptionBuilder",
    skip_from_py_object
)]
pub struct PyEquityOptionBuilder {
    inner: Option<EquityOptionBuilderInner>,
    fields: Vec<(&'static str, String)>,
}

/// Apply one consuming Rust setter and record the field for ``__repr__``.
macro_rules! eq_set {
    ($slf:ident, $field:ident, $repr:expr, $apply:expr) => {{
        let b = take_builder(&mut $slf.inner)?;
        $slf.inner = Some($apply(b));
        $slf.fields.push((stringify!($field), $repr));
        Ok($slf)
    }};
}

#[pymethods]
impl PyEquityOptionBuilder {
    /// Set the instrument identifier.
    ///
    /// Parameters
    /// ----------
    /// value : str
    ///     Unique identifier for the equity option.
    ///
    /// Returns
    /// -------
    /// EquityOptionBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        eq_set!(
            slf,
            id,
            format!("{value:?}"),
            |b: EquityOptionBuilderInner| b.id(InstrumentId::new(value.to_string()))
        )
    }

    /// Set the underlying equity ticker symbol.
    ///
    /// Parameters
    /// ----------
    /// value : str
    ///     Underlying equity ticker symbol.
    ///
    /// Returns
    /// -------
    /// EquityOptionBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn underlying_ticker<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        eq_set!(
            slf,
            underlying_ticker,
            format!("{value:?}"),
            |b: EquityOptionBuilderInner| b.underlying_ticker(value.to_string())
        )
    }

    /// Set the strike price.
    ///
    /// Parameters
    /// ----------
    /// value : float
    ///     Strike price. Must be finite and positive.
    ///
    /// Returns
    /// -------
    /// EquityOptionBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn strike<'py>(mut slf: PyRefMut<'py, Self>, value: f64) -> PyResult<PyRefMut<'py, Self>> {
        eq_set!(
            slf,
            strike,
            float_repr(value),
            |b: EquityOptionBuilderInner| b.strike(value)
        )
    }

    /// Set the option type.
    ///
    /// Parameters
    /// ----------
    /// value : {"call", "put"}
    ///     Option type of the equity option.
    ///
    /// Returns
    /// -------
    /// EquityOptionBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` is not a recognized option type.
    #[pyo3(text_signature = "($self, value)")]
    fn option_type<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let option_type = enum_from_str(value, "option_type")?;
        eq_set!(
            slf,
            option_type,
            format!("{value:?}"),
            |b: EquityOptionBuilderInner| b.option_type(option_type)
        )
    }

    /// Set the exercise style.
    ///
    /// Parameters
    /// ----------
    /// value : {"european", "american", "bermudan"}
    ///     Exercise style of the equity option. Defaults to ``"european"``
    ///     when never set.
    ///
    /// Returns
    /// -------
    /// EquityOptionBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` is not a recognized exercise style.
    #[pyo3(text_signature = "($self, value)")]
    fn exercise_style<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let exercise_style = enum_from_str(value, "exercise_style")?;
        eq_set!(
            slf,
            exercise_style,
            format!("{value:?}"),
            |b: EquityOptionBuilderInner| b.exercise_style(exercise_style)
        )
    }

    /// Set the day basis for per-day theta.
    ///
    /// Parameters
    /// ----------
    /// value : {"calendar_365", "trading_252"}
    ///     Calendar-day theta is the default; trading-day theta must be
    ///     selected explicitly.
    ///
    /// Returns
    /// -------
    /// EquityOptionBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` is not a recognized theta day basis.
    #[pyo3(text_signature = "($self, value)")]
    fn theta_day_basis<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let basis = enum_from_str(value, "theta_day_basis")?;
        eq_set!(
            slf,
            theta_day_basis,
            format!("{value:?}"),
            |b: EquityOptionBuilderInner| b.theta_day_basis(basis)
        )
    }

    /// Set the option expiry date.
    ///
    /// Parameters
    /// ----------
    /// value : datetime.date | str
    ///     Option expiry date (date-like or ISO 8601 string).
    ///
    /// Returns
    /// -------
    /// EquityOptionBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn expiry<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let expiry = extract_date(value)?;
        eq_set!(
            slf,
            expiry,
            date_repr(expiry),
            |b: EquityOptionBuilderInner| b.expiry(expiry)
        )
    }

    /// Set the day count for the time-to-expiry year fraction.
    ///
    /// Parameters
    /// ----------
    /// value : DayCount | str
    ///     Day count convention; defaults to ``ACT/365F`` when never set.
    ///
    /// Returns
    /// -------
    /// EquityOptionBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a string name is not a recognized day count.
    #[pyo3(text_signature = "($self, value)")]
    fn day_count<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let day_count = day_count_from_py(value, "day_count")?;
        eq_set!(
            slf,
            day_count,
            format!("DayCount('{day_count}')"),
            |b: EquityOptionBuilderInner| b.day_count(day_count)
        )
    }

    /// Set the settlement method.
    ///
    /// Parameters
    /// ----------
    /// value : {"physical", "cash"}
    ///     Physical delivery or fixed cash settlement.
    ///
    /// Returns
    /// -------
    /// EquityOptionBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` is not a recognized settlement method.
    #[pyo3(text_signature = "($self, value)")]
    fn settlement<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let settlement = enum_from_str(value, "settlement")?;
        eq_set!(
            slf,
            settlement,
            format!("{value:?}"),
            |b: EquityOptionBuilderInner| b.settlement(settlement)
        )
    }

    /// Set the observed exercise or expiry lifecycle state.
    ///
    /// Parameters
    /// ----------
    /// date : datetime.date | str
    ///     Exercise date, or expiry date for an unexercised observation.
    /// spot : float
    ///     Positive observed underlying level in strike-price units.
    /// settlement_date : datetime.date | str
    ///     Contractual cash-payment or physical-delivery date.
    /// exercised : bool
    ///     Whether exercise or assignment occurred.
    ///
    /// Returns
    /// -------
    /// EquityOptionBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, date, spot, settlement_date, exercised)")]
    fn exercise<'py>(
        mut slf: PyRefMut<'py, Self>,
        date: &Bound<'_, PyAny>,
        spot: f64,
        settlement_date: &Bound<'_, PyAny>,
        exercised: bool,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let exercise = finstack_quant_valuations::instruments::equity::EquityOptionExercise::new(
            extract_date(date)?,
            spot,
            extract_date(settlement_date)?,
            exercised,
        );
        let shown = format!(
            "({}, {}, {}, {})",
            date_repr(exercise.date),
            float_repr(exercise.spot),
            date_repr(exercise.settlement_date),
            bool_repr(exercise.exercised)
        );
        eq_set!(slf, exercise, shown, |b: EquityOptionBuilderInner| b
            .exercise(exercise))
    }

    /// Set the notional amount for valuation scaling.
    ///
    /// Parameters
    /// ----------
    /// value : Money
    ///     Notional amount for valuation scaling.
    ///
    /// Returns
    /// -------
    /// EquityOptionBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn notional<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: PyRef<'_, PyMoney>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let money = value.inner;
        eq_set!(
            slf,
            notional,
            money_repr(money),
            |b: EquityOptionBuilderInner| b.notional(money)
        )
    }

    /// Set the discount curve identifier for present value calculations.
    ///
    /// Parameters
    /// ----------
    /// value : str
    ///     Discount curve identifier.
    ///
    /// Returns
    /// -------
    /// EquityOptionBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn discount_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        eq_set!(
            slf,
            discount_curve_id,
            format!("{value:?}"),
            |b: EquityOptionBuilderInner| b.discount_curve_id(CurveId::new(value.to_string()))
        )
    }

    /// Set the equity spot price identifier.
    ///
    /// Parameters
    /// ----------
    /// value : str
    ///     Equity spot price identifier.
    ///
    /// Returns
    /// -------
    /// EquityOptionBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn spot_id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        eq_set!(
            slf,
            spot_id,
            format!("{value:?}"),
            |b: EquityOptionBuilderInner| b.spot_id(PriceId::new(value.to_string()))
        )
    }

    /// Set the equity volatility surface identifier.
    ///
    /// Parameters
    /// ----------
    /// value : str
    ///     Equity volatility surface identifier.
    ///
    /// Returns
    /// -------
    /// EquityOptionBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn vol_surface_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        eq_set!(
            slf,
            vol_surface_id,
            format!("{value:?}"),
            |b: EquityOptionBuilderInner| b.vol_surface_id(CurveId::new(value.to_string()))
        )
    }

    /// Set the continuous dividend yield identifier.
    ///
    /// Parameters
    /// ----------
    /// value : str
    ///     Continuous dividend yield identifier. If never set, the pricer
    ///     treats the underlying as having zero continuous dividend yield.
    ///
    /// Returns
    /// -------
    /// EquityOptionBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn div_yield_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        eq_set!(
            slf,
            div_yield_id,
            format!("{value:?}"),
            |b: EquityOptionBuilderInner| b.div_yield_id(PriceId::new(value.to_string()))
        )
    }

    /// Set the discrete dividend schedule.
    ///
    /// Parameters
    /// ----------
    /// value : list[tuple[datetime.date | str, float]]
    ///     Positive ``(ex_date, dividend_amount)`` pairs in strictly increasing
    ///     date order. European pricing uses escrowed spot adjustment;
    ///     American/Bermudan tree pricing restores remaining dividend value at
    ///     exercise nodes to model ex-date jumps.
    ///
    /// Returns
    /// -------
    /// EquityOptionBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn discrete_dividends<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Vec<(Bound<'py, PyAny>, f64)>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let dividends = value
            .into_iter()
            .map(|(date, amount)| Ok((extract_date(&date)?, amount)))
            .collect::<PyResult<Vec<_>>>()?;
        let shown = format!(
            "[{}]",
            dividends
                .iter()
                .map(|(d, a)| format!("({}, {})", date_repr(*d), float_repr(*a)))
                .collect::<Vec<_>>()
                .join(", ")
        );
        eq_set!(
            slf,
            discrete_dividends,
            shown,
            |b: EquityOptionBuilderInner| b.discrete_dividends(dividends)
        )
    }

    /// Set the exercise schedule for Bermudan options.
    ///
    /// Parameters
    /// ----------
    /// value : list[datetime.date | str]
    ///     Dates on which early exercise is permitted. Required when
    ///     ``exercise_style`` is ``"bermudan"``.
    ///
    /// Returns
    /// -------
    /// EquityOptionBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn exercise_schedule<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Vec<Bound<'py, PyAny>>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let dates = value
            .iter()
            .map(extract_date)
            .collect::<PyResult<Vec<_>>>()?;
        let shown = format!(
            "[{}]",
            dates
                .iter()
                .map(|d| date_repr(*d))
                .collect::<Vec<_>>()
                .join(", ")
        );
        eq_set!(
            slf,
            exercise_schedule,
            shown,
            |b: EquityOptionBuilderInner| b.exercise_schedule(dates)
        )
    }

    /// Set free-form instrument attributes (tags and metadata).
    ///
    /// Parameters
    /// ----------
    /// value : Attributes | dict[str, str] | None
    ///     Attribute bag; a dict populates metadata, with an optional
    ///     ``"tags"`` list entry populating tags.
    ///
    /// Returns
    /// -------
    /// EquityOptionBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// TypeError
    ///     If ``value`` is neither ``Attributes``, a dict, nor ``None``.
    #[pyo3(text_signature = "($self, value)")]
    fn attributes<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let attrs = attributes_from_py(value)?;
        let shown = value.repr()?.to_string();
        eq_set!(slf, attributes, shown, |b: EquityOptionBuilderInner| b
            .attributes(attrs))
    }

    /// Build the validated equity option.
    ///
    /// Validation is the Rust ``EquityOption::builder().build()`` invariants
    /// only; there is no additional binding-side check.
    ///
    /// Returns
    /// -------
    /// EquityOption
    ///     The validated equity option.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the builder was already consumed, a required field is missing,
    ///     or the completed option fails validation (non-positive strike,
    ///     zero notional, unsorted dividends, inconsistent exercise state).
    #[pyo3(text_signature = "($self)")]
    fn build(mut slf: PyRefMut<'_, Self>) -> PyResult<PyEquityOption> {
        let b = take_builder(&mut slf.inner)?;
        let inner = b.build().map_err(core_to_py)?;
        Ok(PyEquityOption { inner })
    }

    /// Return ``repr(self)`` listing the fields set so far.
    fn __repr__(&self) -> String {
        builder_repr("EquityOptionBuilder", &self.fields)
    }
}

/// Register the typed equity instruments on the instruments submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyEquityOption>()?;
    m.add_class::<PyEquityOptionBuilder>()?;
    Ok(())
}

/// Names this module contributes to `finstack_quant.valuations.instruments.__all__`.
///
/// Extend this list (sorted) when adding a class or function here; `mod.rs`
/// merges every submodule list so registration stays in one place per file.
pub(crate) const EXPORTS: &[&str] = &[];
