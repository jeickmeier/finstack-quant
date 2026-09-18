//! Typed `RevolvingCredit` instrument class and its fluent builder.

use pyo3::prelude::*;

use crate::bindings::cashflows::builder::schedule::PyCashFlowSchedule;
use crate::bindings::core::dates::daycount::PyDayCount;
use crate::bindings::core::dates::schedule::PyStubKind;
use crate::bindings::core::dates::tenor::PyTenor;
use crate::bindings::core::money::PyMoney;
use crate::bindings::core::types::PyAttributes;
use crate::bindings::date_utils::{date_to_py, extract_date};
use crate::bindings::extract::extract_market;
use crate::bindings::pandas_utils::serde_to_py;
use crate::bindings::valuations::convert::{
    attributes_from_py, attributes_to_py, bdc_from_py, builder_repr, day_count_from_py,
    enum_to_py_string, money_from_py, money_repr, money_to_py, tenor_from_py,
};
use crate::bindings::valuations::instruments::{
    instrument_default_model, instrument_expiry, instrument_market_dependencies,
    metric_typed_envelope, parse_typed_instrument_json, price_typed_envelope, pricing_options_json,
    serialize_typed_instrument_json, stub_kind_from_py,
};
use crate::bindings::valuations::PyValuationResult;
use crate::errors::{core_to_py, serde_json_to_py, value_error};
use finstack_quant_cashflows::traits::CashflowScheduleSource;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_valuations::instruments::fixed_income::revolving_credit::{
    BaseRateSpec, DrawRepaySpec, RevolvingCredit, RevolvingCreditFees, RevolvingCreditPricer,
};
use finstack_quant_valuations::instruments::InstrumentJson;

use super::PyEnhancedMonteCarloResult;

type RevolvingCreditBuilderInner =
    finstack_quant_valuations::instruments::fixed_income::revolving_credit::RevolvingCreditBuilder;

/// Parse a nested spec from a ``dict`` (serde shape) or a JSON ``str``.
fn spec_from_py<T: serde::de::DeserializeOwned + Send>(
    py: Python<'_>,
    value: &Bound<'_, PyAny>,
    label: &str,
) -> PyResult<T> {
    if let Ok(json) = value.extract::<String>() {
        return serde_json::from_str(&json)
            .map_err(|err| serde_json_to_py(err, &format!("invalid {label} JSON")));
    }
    crate::bindings::module_utils::py_to_serde(py, value, label)
}

/// Typed wrapper for the Rust `RevolvingCredit` instrument.
///
/// Construct via ``RevolvingCredit.builder()``, the ``RevolvingCredit.example``
/// preset, or ``RevolvingCredit.from_json``. Every public Rust field is
/// readable as a property; ``price`` / ``metric`` run the same pricer as
/// ``price_instrument``. Stochastic facilities (``draw_repay_spec`` of kind
/// ``"stochastic"``) additionally expose ``price_with_paths`` and their
/// path-averaged ``expected_cashflows``.
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "RevolvingCredit",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyRevolvingCredit {
    /// Inner canonical Rust facility.
    pub(crate) inner: RevolvingCredit,
}

impl PyRevolvingCredit {
    /// Serialize as the canonical instrument envelope accepted by the JSON loader.
    pub(crate) fn envelope_json(&self) -> PyResult<String> {
        serialize_typed_instrument_json(
            InstrumentJson::RevolvingCredit(self.inner.clone()),
            "RevolvingCredit",
        )
    }
}

#[pymethods]
impl PyRevolvingCredit {
    /// Create a fluent builder (mirrors Rust ``RevolvingCredit::builder()``).
    ///
    /// Returns
    /// -------
    /// RevolvingCreditBuilder
    ///     A builder with fluent, consuming setter methods.
    ///
    /// Examples
    /// --------
    /// >>> from finstack_quant.valuations.instruments import RevolvingCredit
    /// >>> builder = RevolvingCredit.builder()
    /// >>> builder.id("RCF-1") is builder
    /// True
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn builder() -> PyRevolvingCreditBuilder {
        PyRevolvingCreditBuilder {
            inner: Some(RevolvingCredit::builder()),
            fields: Vec::new(),
        }
    }

    /// Canonical example: USD 50M three-year SOFR + 250bp facility, USD 10M
    /// drawn, with a scheduled draw and repayment (mirrors Rust
    /// ``RevolvingCredit::example``).
    ///
    /// Returns
    /// -------
    /// RevolvingCredit
    ///     The example facility.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If construction fails (should not occur).
    ///
    /// Examples
    /// --------
    /// >>> from finstack_quant.valuations.instruments import RevolvingCredit
    /// >>> RevolvingCredit.example().id
    /// 'RCF-USD-3Y'
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn example() -> PyResult<Self> {
        RevolvingCredit::example()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    /// Support ``pickle`` (and therefore ``multiprocessing``, ``joblib``, ``dask``).
    ///
    /// Reconstruction goes through the same strict serde round-trip as
    /// ``to_json`` / ``from_json``, so an unpickled value is exactly what the
    /// wire format defines.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Deserialize a validated facility from its canonical v1 envelope.
    ///
    /// Parameters
    /// ----------
    /// json : str
    ///     A ``finstack_quant.instrument/1`` envelope containing an exact
    ///     ``"revolving_credit"`` payload. The UTF-8 input must not exceed
    ///     16 MiB. Bare payloads and cross-type coercion are rejected.
    ///
    /// Returns
    /// -------
    /// RevolvingCredit
    ///     The validated facility represented by the payload.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the input exceeds 16 MiB, is malformed, has an unsupported
    ///     envelope schema, carries a type other than ``"revolving_credit"``,
    ///     or fails facility validation.
    ///
    /// Examples
    /// --------
    /// >>> from finstack_quant.valuations.instruments import RevolvingCredit
    /// >>> facility = RevolvingCredit.example()
    /// >>> RevolvingCredit.from_json(facility.to_json()).id == facility.id
    /// True
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        match parse_typed_instrument_json(json)? {
            InstrumentJson::RevolvingCredit(inner) => Ok(Self { inner }),
            _ => Err(value_error(
                "expected instrument type \"revolving_credit\", got a different instrument type",
            )),
        }
    }

    /// Serialize to a canonical ``finstack_quant.instrument/1`` envelope.
    ///
    /// Returns
    /// -------
    /// str
    ///     Envelope accepted by ``price_instrument`` and ``from_json``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If serialization fails.
    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        self.envelope_json()
    }

    /// Serde form of the facility as a Python ``dict``.
    ///
    /// Returns
    /// -------
    /// dict
    ///     Canonical serde shape of the Rust ``RevolvingCredit``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the value cannot be serialized.
    #[pyo3(text_signature = "($self)")]
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner)
    }

    /// Price the facility and return a ``ValuationResult``.
    ///
    /// Same pipeline and keyword surface as ``price_instrument``; a
    /// stochastic ``draw_repay_spec`` prices by Monte Carlo, a deterministic
    /// one on its contractual schedule.
    ///
    /// Parameters
    /// ----------
    /// market : MarketContext | str
    ///     Market context object or JSON string.
    /// as_of : datetime.date | datetime.datetime | pandas.Timestamp | str
    ///     Valuation date.
    /// model : str, default "default"
    ///     Model key.
    /// metrics : list[str], optional
    ///     Metric identifiers to compute (for example ``"dv01"`` or
    ///     ``"draw_option_cost"``).
    /// pricing_options : dict | str, optional
    ///     ``MetricPricingOverrides`` merged into the instrument's overrides.
    /// market_history : str, optional
    ///     JSON ``MarketHistory`` scenarios for ``hvar`` / ``expected_shortfall``.
    ///
    /// Returns
    /// -------
    /// ValuationResult
    ///     Typed valuation envelope.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If an input cannot be interpreted or the instrument fails validation.
    /// KeyError
    ///     If a required curve or metric is missing.
    /// RuntimeError
    ///     If pricing or a metric computation fails.
    #[pyo3(signature = (market, as_of, model="default", metrics=None, pricing_options=None, market_history=None))]
    #[pyo3(
        text_signature = "($self, market, as_of, model='default', metrics=None, pricing_options=None, market_history=None)"
    )]
    // PyO3 binding: the argument list mirrors the Python keyword-argument API.
    #[allow(clippy::too_many_arguments)]
    fn price(
        &self,
        py: Python<'_>,
        market: &Bound<'_, PyAny>,
        as_of: &Bound<'_, PyAny>,
        model: &str,
        metrics: Option<Vec<String>>,
        pricing_options: Option<&Bound<'_, PyAny>>,
        market_history: Option<&str>,
    ) -> PyResult<PyValuationResult> {
        let options = pricing_options_json(py, pricing_options)?;
        price_typed_envelope(
            py,
            self.envelope_json()?,
            market,
            as_of,
            model,
            metrics,
            options,
            market_history,
        )
    }

    /// Compute one scalar metric (e.g. ``"dv01"`` or ``"draw_option_cost"``).
    ///
    /// Parameters
    /// ----------
    /// market : MarketContext | str
    ///     Market context object or JSON string.
    /// as_of : datetime.date | datetime.datetime | pandas.Timestamp | str
    ///     Valuation date.
    /// metric_id : str
    ///     Registered metric identifier.
    /// model : str, default "default"
    ///     Model key.
    ///
    /// Returns
    /// -------
    /// float
    ///     The metric value.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``metric_id`` is unknown or an input cannot be interpreted.
    /// KeyError
    ///     If a required curve is missing.
    /// RuntimeError
    ///     If the metric computation fails.
    #[pyo3(signature = (market, as_of, metric_id, model="default"))]
    #[pyo3(text_signature = "($self, market, as_of, metric_id, model='default')")]
    fn metric(
        &self,
        py: Python<'_>,
        market: &Bound<'_, PyAny>,
        as_of: &Bound<'_, PyAny>,
        metric_id: &str,
        model: &str,
    ) -> PyResult<f64> {
        metric_typed_envelope(py, self.envelope_json()?, market, as_of, metric_id, model)
    }

    /// Run the stochastic Monte Carlo valuation and keep every path.
    ///
    /// Parameters
    /// ----------
    /// market : MarketContext | str
    ///     Market context with the discount curve, the index forward curve
    ///     and fixings for floating facilities, and the hazard curve for
    ///     market-anchored spread processes.
    /// as_of : datetime.date | datetime.datetime | pandas.Timestamp | str
    ///     Valuation date; the simulation starts here with the current drawn
    ///     amount.
    ///
    /// Returns
    /// -------
    /// EnhancedMonteCarloResult
    ///     Present value and draw option cost estimates plus every path.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the facility has a deterministic ``draw_repay_spec`` or fails
    ///     validation.
    /// KeyError
    ///     If a required curve is missing from ``market``.
    /// RuntimeError
    ///     If the simulation fails.
    #[pyo3(text_signature = "($self, market, as_of)")]
    fn price_with_paths(
        &self,
        py: Python<'_>,
        market: &Bound<'_, PyAny>,
        as_of: &Bound<'_, PyAny>,
    ) -> PyResult<PyEnhancedMonteCarloResult> {
        let market = extract_market(py, market)?;
        let as_of = extract_date(as_of)?;
        let facility = self.inner.clone();
        let inner = py
            .detach(move || RevolvingCreditPricer::price_with_paths(&facility, &market, as_of))
            .map_err(core_to_py)?;
        Ok(PyEnhancedMonteCarloResult { inner })
    }

    /// Cashflow schedule of the facility: the contractual schedule for a
    /// deterministic ``draw_repay_spec``, the path-averaged schedule for a
    /// stochastic one.
    ///
    /// Parameters
    /// ----------
    /// market : MarketContext | str
    ///     Market context with the curves the schedule projects from.
    /// as_of : datetime.date | datetime.datetime | pandas.Timestamp | str
    ///     Valuation date; flows are projected from this date.
    ///
    /// Returns
    /// -------
    /// CashFlowSchedule
    ///     Dated interest, fee and principal flows from the lender's
    ///     perspective (draws negative, repayments positive).
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the facility fails validation or the schedule cannot be built.
    /// KeyError
    ///     If a required curve is missing from ``market``.
    #[pyo3(text_signature = "($self, market, as_of)")]
    fn expected_cashflows(
        &self,
        py: Python<'_>,
        market: &Bound<'_, PyAny>,
        as_of: &Bound<'_, PyAny>,
    ) -> PyResult<PyCashFlowSchedule> {
        let market = extract_market(py, market)?;
        let as_of = extract_date(as_of)?;
        let facility = self.inner.clone();
        let schedule = py
            .detach(move || facility.raw_cashflow_schedule(&market, as_of))
            .map_err(core_to_py)?;
        Ok(PyCashFlowSchedule::from_inner(schedule))
    }

    /// Instrument identifier.
    #[getter]
    fn id(&self) -> String {
        self.inner.id.to_string()
    }

    /// Total committed amount.
    #[getter]
    fn commitment_amount(&self) -> PyMoney {
        money_to_py(self.inner.commitment_amount)
    }

    /// Drawn balance at the simulation anchor (the later of the commitment
    /// and valuation dates), in both deterministic and stochastic mode.
    #[getter]
    fn drawn_amount(&self) -> PyMoney {
        money_to_py(self.inner.drawn_amount)
    }

    /// Date the facility becomes available, as ``datetime.date``.
    #[getter]
    fn commitment_date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.commitment_date)
    }

    /// Expiry of the commitment, as ``datetime.date``.
    #[getter]
    fn maturity<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.maturity)
    }

    /// Base-rate specification as its serde ``dict`` (``{"fixed": {"rate": r}}``
    /// or ``{"floating": {...}}``).
    #[getter]
    fn base_rate_spec<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.base_rate_spec)
    }

    /// Interest accrual day count.
    #[getter]
    fn day_count(&self) -> PyDayCount {
        PyDayCount::from_inner(self.inner.day_count)
    }

    /// Payment frequency for interest and fees.
    #[getter]
    fn frequency(&self) -> PyTenor {
        PyTenor::from_inner(self.inner.frequency)
    }

    /// Fee structure as its serde ``dict`` (including dated ``steps``).
    #[getter]
    fn fees<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.fees)
    }

    /// Scheduled commitment changes as a ``list`` of serde ``dict`` rows
    /// (``date``, ``amount``, ``fee_bp``), empty when the commitment is flat.
    #[getter]
    fn commitment_schedule<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.commitment_schedule)
    }

    /// Dated margin steps as a ``list`` of serde ``dict`` rows (``date``,
    /// ``delta_bp``), empty when the margin is flat.
    #[getter]
    fn margin_steps<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.margin_steps)
    }

    /// Dated fixed fees as a ``list`` of serde ``dict`` rows (``date``,
    /// ``amount``), empty when none are scheduled.
    #[getter]
    fn scheduled_fees<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.scheduled_fees)
    }

    /// Effective-interest-rate reporting switch as its serde ``dict``
    /// (``{"include_fees": bool}``), or ``None`` for the default (fees
    /// included).
    #[getter]
    fn oid_eir<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .oid_eir
            .as_ref()
            .map(|spec| serde_to_py(py, spec))
            .transpose()
    }

    /// Letter-of-credit sub-facility as its serde ``dict`` (``sublimit``,
    /// ``outstanding``, ``events``, ``fee_bp``, ``fronting_fee_bp``,
    /// ``leq``), or ``None`` when the facility has no LC sublimit.
    #[getter]
    fn lc<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .lc
            .as_ref()
            .map(|lc| serde_to_py(py, lc))
            .transpose()
    }

    /// Draw/repay specification as its serde ``dict`` (``{"deterministic":
    /// [...]}`` or ``{"stochastic": {...}}``).
    #[getter]
    fn draw_repay_spec<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.draw_repay_spec)
    }

    /// ``True`` when utilization is simulated (stochastic ``draw_repay_spec``).
    #[getter]
    fn is_stochastic(&self) -> bool {
        self.inner.is_stochastic()
    }

    /// Discount curve identifier.
    #[getter]
    fn discount_curve_id(&self) -> String {
        self.inner.discount_curve_id.to_string()
    }

    /// Credit (hazard) curve identifier, or ``None``.
    #[getter]
    fn credit_curve_id(&self) -> Option<String> {
        self.inner.credit_curve_id.as_ref().map(ToString::to_string)
    }

    /// Recovery rate on default, as a decimal in ``[0, 1]``.
    #[getter]
    fn recovery_rate(&self) -> f64 {
        self.inner.recovery_rate
    }

    /// Loan-equivalent exposure: fraction of the undrawn commitment drawn at
    /// default, as a decimal in ``[0, 1]``.
    #[getter]
    fn leq(&self) -> f64 {
        self.inner.leq
    }

    /// Stub rule for schedule generation.
    #[getter]
    fn stub(&self) -> PyStubKind {
        PyStubKind::from_inner(self.inner.stub)
    }

    /// Business-day convention applied to payment dates (serde string, e.g.
    /// ``"modified_following"``).
    #[getter]
    fn business_day_convention(&self) -> PyResult<String> {
        enum_to_py_string(&self.inner.business_day_convention)
    }

    /// Holiday calendar identifier used for payment, fixing and settlement
    /// rolls, or ``None`` for weekends only.
    #[getter]
    fn calendar_id(&self) -> Option<String> {
        self.inner.calendar_id.clone()
    }

    /// Business days between an accrual end and its payment date.
    #[getter]
    fn payment_lag_days(&self) -> u32 {
        self.inner.payment_lag_days
    }

    /// Business days from the valuation date to the settlement date used by
    /// quote metrics.
    #[getter]
    fn settlement_days(&self) -> u32 {
        self.inner.settlement_days
    }

    /// Scenario-selection attributes.
    #[getter]
    fn attributes(&self) -> PyAttributes {
        attributes_to_py(&self.inner.attributes)
    }

    /// Default pricing model key from the ``Instrument`` trait.
    #[getter]
    fn default_model(&self) -> String {
        instrument_default_model(&self.inner)
    }

    /// Expiry date exposed by the ``Instrument`` trait, or ``None``.
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        instrument_expiry(py, &self.inner)
    }

    /// Market-data dependencies (curves, fixings) as a dict.
    ///
    /// Returns
    /// -------
    /// dict
    ///     Serde form of the Rust ``MarketDependencies``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the instrument cannot enumerate its dependencies.
    #[pyo3(text_signature = "($self)")]
    fn market_dependencies<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        instrument_market_dependencies(py, &self.inner)
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "RevolvingCredit(id={:?}, commitment_amount={}, drawn_amount={}, maturity={}, stochastic={})",
            self.inner.id.as_str(),
            money_repr(self.inner.commitment_amount),
            money_repr(self.inner.drawn_amount),
            self.inner.maturity,
            self.inner.is_stochastic(),
        )
    }
}

/// Fluent builder for ``RevolvingCredit``; wraps the Rust
/// `FinancialBuilder`-generated builder (consuming setters).
///
/// Builders are consumed by ``build()``; create a new builder per facility.
/// Nested specs (``base_rate_spec`` when floating, ``fees``,
/// ``draw_repay_spec``) accept a ``dict`` or a JSON ``str`` in the Rust
/// serde shape.
///
/// Examples
/// --------
/// >>> import datetime
/// >>> from finstack_quant.core.currency import Currency
/// >>> from finstack_quant.core.money import Money
/// >>> from finstack_quant.valuations.instruments import RevolvingCredit
/// >>> facility = (
/// ...     RevolvingCredit.builder()
/// ...     .id("RCF-1")
/// ...     .commitment_amount(Money(50_000_000.0, Currency("USD")))
/// ...     .drawn_amount(Money(10_000_000.0, Currency("USD")))
/// ...     .commitment_date(datetime.date(2024, 1, 15))
/// ...     .maturity(datetime.date(2027, 1, 15))
/// ...     .base_rate_spec(0.06)
/// ...     .day_count("act_360")
/// ...     .frequency("3M")
/// ...     .fees_flat(25.0, 10.0, 5.0)
/// ...     .draw_repay_spec({"deterministic": []})
/// ...     .discount_curve_id("USD-OIS")
/// ...     .recovery_rate(0.4)
/// ...     .build()
/// ... )
/// >>> facility.is_stochastic
/// False
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "RevolvingCreditBuilder",
    skip_from_py_object
)]
pub struct PyRevolvingCreditBuilder {
    inner: Option<RevolvingCreditBuilderInner>,
    fields: Vec<(&'static str, String)>,
}

/// Take the wrapped Rust builder or fail if `build()` already consumed it.
fn take_builder(b: &mut PyRevolvingCreditBuilder) -> PyResult<RevolvingCreditBuilderInner> {
    b.inner
        .take()
        .ok_or_else(|| value_error("builder already consumed by build()"))
}

#[pymethods]
impl PyRevolvingCreditBuilder {
    /// Set the instrument identifier.
    ///
    /// Parameters
    /// ----------
    /// value : str
    ///     Unique identifier for the facility.
    ///
    /// Returns
    /// -------
    /// RevolvingCreditBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the builder was already consumed by ``build()``.
    #[pyo3(text_signature = "($self, value)")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let b = take_builder(&mut slf)?;
        slf.inner = Some(b.id(InstrumentId::new(value.to_string())));
        slf.fields.push(("id", format!("{value:?}")));
        Ok(slf)
    }

    /// Set the total commitment.
    ///
    /// Parameters
    /// ----------
    /// value : Money | float
    ///     Commitment; a bare number needs ``currency``.
    /// currency : str, optional
    ///     ISO-4217 code applied when ``value`` is a bare number.
    ///
    /// Returns
    /// -------
    /// RevolvingCreditBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the amount is not finite, a bare number has no currency, or
    ///     the builder was already consumed.
    /// TypeError
    ///     If ``value`` is neither ``Money`` nor a number.
    #[pyo3(signature = (value, currency=None))]
    #[pyo3(text_signature = "($self, value, currency=None)")]
    fn commitment_amount<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
        currency: Option<&str>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let money = money_from_py(value, currency, "commitment_amount")?;
        let b = take_builder(&mut slf)?;
        slf.inner = Some(b.commitment_amount(money));
        slf.fields.push(("commitment_amount", money_repr(money)));
        Ok(slf)
    }

    /// Set the drawn balance at the simulation anchor, the later of the
    /// commitment date and the valuation date, in both modes. Deterministic
    /// draw/repay events must be dated after that anchor.
    ///
    /// Parameters
    /// ----------
    /// value : Money | float
    ///     Drawn balance; a bare number needs ``currency``.
    /// currency : str, optional
    ///     ISO-4217 code applied when ``value`` is a bare number.
    ///
    /// Returns
    /// -------
    /// RevolvingCreditBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the amount is not finite, a bare number has no currency, or
    ///     the builder was already consumed.
    /// TypeError
    ///     If ``value`` is neither ``Money`` nor a number.
    #[pyo3(signature = (value, currency=None))]
    #[pyo3(text_signature = "($self, value, currency=None)")]
    fn drawn_amount<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
        currency: Option<&str>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let money = money_from_py(value, currency, "drawn_amount")?;
        let b = take_builder(&mut slf)?;
        slf.inner = Some(b.drawn_amount(money));
        slf.fields.push(("drawn_amount", money_repr(money)));
        Ok(slf)
    }

    /// Set the date the facility becomes available.
    ///
    /// Parameters
    /// ----------
    /// value : datetime.date | datetime.datetime | pandas.Timestamp | str
    ///     Commitment date (ISO 8601 strings accepted).
    ///
    /// Returns
    /// -------
    /// RevolvingCreditBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` is not a date or the builder was already consumed.
    #[pyo3(text_signature = "($self, value)")]
    fn commitment_date<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let date = extract_date(value)?;
        let b = take_builder(&mut slf)?;
        slf.inner = Some(b.commitment_date(date));
        slf.fields.push(("commitment_date", date.to_string()));
        Ok(slf)
    }

    /// Set the expiry of the commitment.
    ///
    /// Parameters
    /// ----------
    /// value : datetime.date | datetime.datetime | pandas.Timestamp | str
    ///     Maturity date (ISO 8601 strings accepted).
    ///
    /// Returns
    /// -------
    /// RevolvingCreditBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` is not a date or the builder was already consumed.
    #[pyo3(text_signature = "($self, value)")]
    fn maturity<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let date = extract_date(value)?;
        let b = take_builder(&mut slf)?;
        slf.inner = Some(b.maturity(date));
        slf.fields.push(("maturity", date.to_string()));
        Ok(slf)
    }

    /// Set the base rate.
    ///
    /// Parameters
    /// ----------
    /// value : float | dict | str
    ///     A bare decimal builds a fixed rate (``0.06`` = 6%); a ``dict`` or
    ///     JSON ``str`` in the ``BaseRateSpec`` serde shape
    ///     (``{"fixed": {"rate": 0.06}}`` or ``{"floating": {...}}`` with a
    ///     ``FloatingRateSpec``) is used verbatim.
    ///
    /// Returns
    /// -------
    /// RevolvingCreditBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the spec does not match the serde shape or the builder was
    ///     already consumed.
    #[pyo3(text_signature = "($self, value)")]
    fn base_rate_spec<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'py>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let spec: BaseRateSpec = if let Ok(rate) = value.extract::<f64>() {
            BaseRateSpec::Fixed { rate }
        } else {
            spec_from_py(py, value, "base_rate_spec")?
        };
        let repr = serde_json::to_string(&spec).unwrap_or_default();
        let b = take_builder(&mut slf)?;
        slf.inner = Some(b.base_rate_spec(spec));
        slf.fields.push(("base_rate_spec", repr));
        Ok(slf)
    }

    /// Set the interest accrual day count.
    ///
    /// Parameters
    /// ----------
    /// value : DayCount | str
    ///     Day count object or serde name (``"act_360"``, ``"act_365f"``, ``"30_360"``, …).
    ///
    /// Returns
    /// -------
    /// RevolvingCreditBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the name is unknown or the builder was already consumed.
    /// TypeError
    ///     If ``value`` is neither ``DayCount`` nor ``str``.
    #[pyo3(text_signature = "($self, value)")]
    fn day_count<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let day_count = day_count_from_py(value, "day_count")?;
        let b = take_builder(&mut slf)?;
        slf.inner = Some(b.day_count(day_count));
        slf.fields.push(("day_count", format!("{day_count:?}")));
        Ok(slf)
    }

    /// Set the payment frequency for interest and fees.
    ///
    /// Parameters
    /// ----------
    /// value : Tenor | str
    ///     Tenor object or string such as ``"3M"``.
    ///
    /// Returns
    /// -------
    /// RevolvingCreditBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the tenor cannot be parsed or the builder was already consumed.
    /// TypeError
    ///     If ``value`` is neither ``Tenor`` nor ``str``.
    #[pyo3(text_signature = "($self, value)")]
    fn frequency<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let tenor = tenor_from_py(value, "frequency")?;
        let b = take_builder(&mut slf)?;
        slf.inner = Some(b.frequency(tenor));
        slf.fields.push(("frequency", format!("{tenor:?}")));
        Ok(slf)
    }

    /// Set the fee structure from its serde shape.
    ///
    /// Parameters
    /// ----------
    /// value : dict | str
    ///     ``RevolvingCreditFees`` as a ``dict`` or JSON ``str``
    ///     (``upfront_fee`` as ``None``, ``{"amount": Money-dict}`` or
    ///     ``{"pct_of_commitment": 0.02}``; ``commitment_fee_tiers``,
    ///     ``usage_fee_tiers``, ``facility_fee_bp`` and the dated ``steps`` list of
    ///     ``{"date", "commitment_delta_bp", "usage_delta_bp",
    ///     "facility_delta_bp"}`` rows).
    ///
    /// Returns
    /// -------
    /// RevolvingCreditBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the spec does not match the serde shape or the builder was
    ///     already consumed.
    #[pyo3(text_signature = "($self, value)")]
    fn fees<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'py>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let fees: RevolvingCreditFees = spec_from_py(py, value, "fees")?;
        let b = take_builder(&mut slf)?;
        slf.inner = Some(b.fees(fees));
        slf.fields.push(("fees", "{..}".to_string()));
        Ok(slf)
    }

    /// Set the scheduled commitment changes.
    ///
    /// Parameters
    /// ----------
    /// value : list[dict[str, Any]] | str
    ///     Rows of ``{"date": "YYYY-MM-DD", "amount": Money-dict, "fee_bp":
    ///     float}`` (a ``list`` of dicts or a JSON ``str``), each the
    ///     commitment in force from its date; ``fee_bp`` is the reduction fee
    ///     on a step down, in basis points of the reduced amount. Dates must
    ///     be strictly increasing, after the commitment date and on or before
    ///     maturity.
    ///
    /// Returns
    /// -------
    /// RevolvingCreditBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a row does not match the serde shape or the builder was
    ///     already consumed; ordering and feasibility fail at ``build()``.
    #[pyo3(text_signature = "($self, value)")]
    fn commitment_schedule<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'py>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let steps: Vec<
            finstack_quant_valuations::instruments::fixed_income::loan_terms::CommitmentStep,
        > = spec_from_py(py, value, "commitment_schedule")?;
        let b = take_builder(&mut slf)?;
        slf.inner = Some(b.commitment_schedule(steps));
        slf.fields.push(("commitment_schedule", "[..]".to_string()));
        Ok(slf)
    }

    /// Set the dated fixed fees (amendment, waiver, extension, consent).
    ///
    /// Parameters
    /// ----------
    /// value : list[dict[str, Any]] | str
    ///     Rows of ``{"date": "YYYY-MM-DD", "amount": Money-dict}`` (a
    ///     ``list`` of dicts or a JSON ``str``), each paid on its date. Dates
    ///     must lie after the commitment date and on or before maturity.
    ///
    /// Returns
    /// -------
    /// RevolvingCreditBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a row does not match the serde shape or the builder was
    ///     already consumed; date and sign checks fail at ``build()``.
    #[pyo3(text_signature = "($self, value)")]
    fn scheduled_fees<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'py>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let fees: Vec<
            finstack_quant_valuations::instruments::fixed_income::loan_terms::ScheduledFee,
        > = spec_from_py(py, value, "scheduled_fees")?;
        let b = take_builder(&mut slf)?;
        slf.inner = Some(b.scheduled_fees(fees));
        slf.fields.push(("scheduled_fees", "[..]".to_string()));
        Ok(slf)
    }

    /// Set the effective-interest-rate reporting switch.
    ///
    /// Parameters
    /// ----------
    /// value : dict[str, Any] | str | None
    ///     ``{"include_fees": bool}`` as a ``dict`` or JSON ``str``; ``None``
    ///     (the default) includes fees in the effective yield.
    ///
    /// Returns
    /// -------
    /// RevolvingCreditBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the spec does not match the serde shape or the builder was
    ///     already consumed.
    #[pyo3(signature = (value))]
    #[pyo3(text_signature = "($self, value)")]
    fn oid_eir<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'py>,
        value: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let spec: Option<
            finstack_quant_valuations::instruments::fixed_income::loan_terms::OidEirSpec,
        > = value.map(|v| spec_from_py(py, v, "oid_eir")).transpose()?;
        let repr = if spec.is_some() { "{..}" } else { "None" };
        let b = take_builder(&mut slf)?;
        slf.inner = Some(b.oid_eir_opt(spec));
        slf.fields.push(("oid_eir", repr.to_string()));
        Ok(slf)
    }

    /// Set the letter-of-credit sub-facility.
    ///
    /// Parameters
    /// ----------
    /// value : dict[str, Any] | str | None
    ///     ``LetterOfCreditSpec`` as a ``dict`` or JSON ``str`` with
    ///     ``sublimit`` and ``outstanding`` (Money dicts), ``events`` (rows
    ///     of ``{"date", "amount", "is_issue"}``), ``fee_bp`` (``None``
    ///     accrues the floating margin), ``fronting_fee_bp`` and ``leq`` (the
    ///     fraction of the LC face drawn at default). ``None`` removes the
    ///     sublimit.
    ///
    /// Returns
    /// -------
    /// RevolvingCreditBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the spec does not match the serde shape or the builder was
    ///     already consumed; sublimit and capacity checks fail at ``build()``.
    #[pyo3(signature = (value))]
    #[pyo3(text_signature = "($self, value)")]
    fn lc<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'py>,
        value: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let lc: Option<
            finstack_quant_valuations::instruments::fixed_income::loan_terms::LetterOfCreditSpec,
        > = value.map(|v| spec_from_py(py, v, "lc")).transpose()?;
        let repr = if lc.is_some() { "{..}" } else { "None" };
        let b = take_builder(&mut slf)?;
        slf.inner = Some(b.lc_opt(lc));
        slf.fields.push(("lc", repr.to_string()));
        Ok(slf)
    }

    /// Set the dated margin steps.
    ///
    /// Parameters
    /// ----------
    /// value : list[dict[str, Any]] | str
    ///     Rows of ``{"date": "YYYY-MM-DD", "delta_bp": int}`` (a ``list`` of
    ///     dicts or a JSON ``str``); each delta shifts the floating spread or
    ///     the fixed rate from its date, cumulatively. Dates must be strictly
    ///     increasing and strictly inside the facility life.
    ///
    /// Returns
    /// -------
    /// RevolvingCreditBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a row does not match the serde shape or the builder was
    ///     already consumed; ordering fails at ``build()``.
    #[pyo3(text_signature = "($self, value)")]
    fn margin_steps<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'py>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let steps: Vec<
            finstack_quant_valuations::instruments::fixed_income::loan_terms::MarginStepUp,
        > = spec_from_py(py, value, "margin_steps")?;
        let b = take_builder(&mut slf)?;
        slf.inner = Some(b.margin_steps(steps));
        slf.fields.push(("margin_steps", "[..]".to_string()));
        Ok(slf)
    }

    /// Set flat (non-tiered) fees in basis points (mirrors Rust
    /// ``RevolvingCreditFees::flat``).
    ///
    /// Parameters
    /// ----------
    /// commitment_fee_bp : float
    ///     Annual commitment fee on the undrawn amount, in basis points;
    ///     values ``<= 0`` produce no commitment fee.
    /// usage_fee_bp : float
    ///     Annual usage fee on the drawn amount, in basis points.
    /// facility_fee_bp : float
    ///     Annual facility fee on the total commitment, in basis points.
    ///
    /// Returns
    /// -------
    /// RevolvingCreditBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a rate is not finite or the builder was already consumed.
    #[pyo3(text_signature = "($self, commitment_fee_bp, usage_fee_bp, facility_fee_bp)")]
    fn fees_flat<'py>(
        mut slf: PyRefMut<'py, Self>,
        commitment_fee_bp: f64,
        usage_fee_bp: f64,
        facility_fee_bp: f64,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let fees = RevolvingCreditFees::flat(commitment_fee_bp, usage_fee_bp, facility_fee_bp)
            .map_err(core_to_py)?;
        let b = take_builder(&mut slf)?;
        slf.inner = Some(b.fees(fees));
        slf.fields.push((
            "fees",
            format!("flat({commitment_fee_bp}, {usage_fee_bp}, {facility_fee_bp})"),
        ));
        Ok(slf)
    }

    /// Set the draw/repay specification from its serde shape.
    ///
    /// Parameters
    /// ----------
    /// value : dict | str
    ///     ``DrawRepaySpec`` as a ``dict`` or JSON ``str``:
    ///     ``{"deterministic": [{"date": ..., "amount": Money, "is_draw": bool}, ...]}``
    ///     or ``{"stochastic": {"utilization_process": {...}, "num_paths": ..., ...}}``.
    ///
    /// Returns
    /// -------
    /// RevolvingCreditBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the spec does not match the serde shape or the builder was
    ///     already consumed.
    #[pyo3(text_signature = "($self, value)")]
    fn draw_repay_spec<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'py>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let spec: DrawRepaySpec = spec_from_py(py, value, "draw_repay_spec")?;
        let kind = match &spec {
            DrawRepaySpec::Deterministic(_) => "deterministic",
            DrawRepaySpec::Stochastic(_) => "stochastic",
        };
        let b = take_builder(&mut slf)?;
        slf.inner = Some(b.draw_repay_spec(spec));
        slf.fields.push(("draw_repay_spec", kind.to_string()));
        Ok(slf)
    }

    /// Set the discount curve identifier.
    ///
    /// Parameters
    /// ----------
    /// value : str
    ///     Discount curve id in the market context.
    ///
    /// Returns
    /// -------
    /// RevolvingCreditBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the builder was already consumed.
    #[pyo3(text_signature = "($self, value)")]
    fn discount_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let b = take_builder(&mut slf)?;
        slf.inner = Some(b.discount_curve_id(CurveId::new(value.to_string())));
        slf.fields.push(("discount_curve_id", format!("{value:?}")));
        Ok(slf)
    }

    /// Set (or clear) the credit curve identifier.
    ///
    /// Parameters
    /// ----------
    /// value : str | None
    ///     Hazard curve id used for survival weighting; ``None`` prices
    ///     without credit risk. A stochastic facility with a credit curve
    ///     must use a market-anchored spread process on the same curve.
    ///
    /// Returns
    /// -------
    /// RevolvingCreditBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the builder was already consumed.
    #[pyo3(signature = (value))]
    #[pyo3(text_signature = "($self, value)")]
    fn credit_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<&str>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let b = take_builder(&mut slf)?;
        slf.inner = Some(b.credit_curve_id_opt(value.map(|v| CurveId::new(v.to_string()))));
        slf.fields.push(("credit_curve_id", format!("{value:?}")));
        Ok(slf)
    }

    /// Set the recovery rate on default.
    ///
    /// Parameters
    /// ----------
    /// value : float
    ///     Recovery fraction as a decimal in ``[0, 1]``.
    ///
    /// Returns
    /// -------
    /// RevolvingCreditBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the builder was already consumed.
    #[pyo3(text_signature = "($self, value)")]
    fn recovery_rate<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: f64,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let b = take_builder(&mut slf)?;
        slf.inner = Some(b.recovery_rate(value));
        slf.fields.push(("recovery_rate", value.to_string()));
        Ok(slf)
    }

    /// Set the loan-equivalent exposure drawn at default.
    ///
    /// Parameters
    /// ----------
    /// value : float
    ///     Fraction of the undrawn commitment assumed drawn at default, as a
    ///     decimal in ``[0, 1]`` (default ``0.0``).
    ///
    /// Returns
    /// -------
    /// RevolvingCreditBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the builder was already consumed.
    #[pyo3(text_signature = "($self, value)")]
    fn leq<'py>(mut slf: PyRefMut<'py, Self>, value: f64) -> PyResult<PyRefMut<'py, Self>> {
        let b = take_builder(&mut slf)?;
        slf.inner = Some(b.leq(value));
        slf.fields.push(("leq", value.to_string()));
        Ok(slf)
    }

    /// Set the stub rule for schedule generation.
    ///
    /// Parameters
    /// ----------
    /// value : StubKind | str
    ///     Stub kind object or serde name (``"short_front"``, ``"short_back"``,
    ///     ``"long_front"``, ``"long_back"``, ``"none"``); default
    ///     ``"short_front"``.
    ///
    /// Returns
    /// -------
    /// RevolvingCreditBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the name is unknown or the builder was already consumed.
    /// TypeError
    ///     If ``value`` is neither ``StubKind`` nor ``str``.
    #[pyo3(text_signature = "($self, value)")]
    fn stub<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let stub = stub_kind_from_py(Some(value), "stub")?;
        let b = take_builder(&mut slf)?;
        slf.inner = Some(b.stub(stub));
        slf.fields.push(("stub", format!("{stub:?}")));
        Ok(slf)
    }

    /// Set the business-day convention for payment dates.
    ///
    /// Parameters
    /// ----------
    /// value : BusinessDayConvention | str
    ///     Convention object or serde name (``"modified_following"``,
    ///     ``"following"``, ``"preceding"``, ...); default
    ///     ``"modified_following"``. Accrual boundaries stay unadjusted.
    ///
    /// Returns
    /// -------
    /// RevolvingCreditBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the name is unknown or the builder was already consumed.
    /// TypeError
    ///     If ``value`` is neither ``BusinessDayConvention`` nor ``str``.
    #[pyo3(text_signature = "($self, value)")]
    fn business_day_convention<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let convention = bdc_from_py(value, "business_day_convention")?;
        let b = take_builder(&mut slf)?;
        slf.inner = Some(b.business_day_convention(convention));
        slf.fields
            .push(("business_day_convention", format!("{convention:?}")));
        Ok(slf)
    }

    /// Set the holiday calendar used for payment, fixing and settlement rolls.
    ///
    /// Parameters
    /// ----------
    /// value : str | None
    ///     Calendar identifier such as ``"usny"``; ``None`` (the default)
    ///     adjusts for weekends only. An unknown identifier fails at
    ///     ``build()``.
    ///
    /// Returns
    /// -------
    /// RevolvingCreditBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the builder was already consumed.
    #[pyo3(signature = (value))]
    #[pyo3(text_signature = "($self, value)")]
    fn calendar_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<String>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let b = take_builder(&mut slf)?;
        slf.inner = Some(b.calendar_id_opt(value.clone()));
        slf.fields.push(("calendar_id", format!("{value:?}")));
        Ok(slf)
    }

    /// Set the payment lag in business days after each accrual end.
    ///
    /// Parameters
    /// ----------
    /// value : int
    ///     Business days on the facility calendar; ``0`` (the default) pays
    ///     on the adjusted accrual end.
    ///
    /// Returns
    /// -------
    /// RevolvingCreditBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the builder was already consumed.
    #[pyo3(text_signature = "($self, value)")]
    fn payment_lag_days<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: u32,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let b = take_builder(&mut slf)?;
        slf.inner = Some(b.payment_lag_days(value));
        slf.fields.push(("payment_lag_days", value.to_string()));
        Ok(slf)
    }

    /// Set the settlement lag used by quote metrics.
    ///
    /// Parameters
    /// ----------
    /// value : int
    ///     Business days from the valuation date to settlement; ``0`` (the
    ///     default) settles on the valuation date. The base present value is
    ///     always anchored at the valuation date.
    ///
    /// Returns
    /// -------
    /// RevolvingCreditBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the builder was already consumed.
    #[pyo3(text_signature = "($self, value)")]
    fn settlement_days<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: u32,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let b = take_builder(&mut slf)?;
        slf.inner = Some(b.settlement_days(value));
        slf.fields.push(("settlement_days", value.to_string()));
        Ok(slf)
    }

    /// Set scenario-selection attributes.
    ///
    /// Parameters
    /// ----------
    /// value : Attributes | dict[str, str] | None
    ///     Attribute map; ``None`` clears it.
    ///
    /// Returns
    /// -------
    /// RevolvingCreditBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the builder was already consumed.
    /// TypeError
    ///     If ``value`` is neither ``Attributes`` nor a ``dict``.
    #[pyo3(text_signature = "($self, value)")]
    fn attributes<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let attributes = attributes_from_py(value)?;
        let b = take_builder(&mut slf)?;
        slf.inner = Some(b.attributes(attributes));
        slf.fields.push(("attributes", "{..}".to_string()));
        Ok(slf)
    }

    /// Consume the builder and validate the facility.
    ///
    /// Returns
    /// -------
    /// RevolvingCredit
    ///     The validated facility.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the builder was already consumed, a required field is missing
    ///     (the message names the field), or the facility fails validation.
    #[pyo3(text_signature = "($self)")]
    fn build(mut slf: PyRefMut<'_, Self>) -> PyResult<PyRevolvingCredit> {
        let b = take_builder(&mut slf)?;
        let inner = b.build().map_err(core_to_py)?;
        Ok(PyRevolvingCredit { inner })
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        builder_repr("RevolvingCreditBuilder", &self.fields)
    }
}
