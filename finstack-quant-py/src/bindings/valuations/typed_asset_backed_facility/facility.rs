//! `AssetBackedFacility`, `AssetBackedFacilityBuilder` and `FacilityProjection`.

use pyo3::prelude::*;

use crate::bindings::cashflows::builder::specs::{
    PyDefaultModelSpec, PyPrepaymentModelSpec, PyRecoveryModelSpec,
};
use crate::bindings::core::dates::daycount::PyDayCount;
use crate::bindings::core::dates::tenor::PyTenor;
use crate::bindings::core::money::PyMoney;
use crate::bindings::core::types::PyAttributes;
use crate::bindings::date_utils::{date_to_py, extract_date};
use crate::bindings::extract::extract_market;
use crate::bindings::pandas_utils::{serde_rows_to_dataframe_with_schema, serde_to_py};
use crate::bindings::valuations::convert::{
    attributes_from_py, attributes_to_py, bool_repr, day_count_from_py, money_from_py, money_to_py,
    tenor_from_py,
};
use crate::bindings::valuations::instruments::{
    instrument_default_model, instrument_market_dependencies, metric_typed_envelope,
    parse_typed_instrument_json, price_typed_envelope, pricing_options_json,
    serialize_typed_instrument_json,
};
use crate::bindings::valuations::typed_structured_credit::{
    PyAssetPool, PySimulationDiagnostics, PyStructuredCredit, PyTrancheCashflows,
};
use crate::bindings::valuations::PyValuationResult;
use crate::errors::{core_to_py, display_to_py, serde_json_to_py, value_error};
use finstack_quant_cashflows::builder::{DefaultModelSpec, PrepaymentModelSpec, RecoveryModelSpec};
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_valuations::instruments::fixed_income::asset_backed_facility::{
    AmortizationEvent, AssetBackedFacility, BorrowingBaseRules, FacilityDraw, FacilityProjection,
    TermOutSpec,
};
use finstack_quant_valuations::instruments::fixed_income::structured_credit::CreditModelConfig;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::DealFees;
use finstack_quant_valuations::instruments::InstrumentJson;

type FacilityBuilderInner =
    finstack_quant_valuations::instruments::fixed_income::asset_backed_facility::AssetBackedFacilityBuilder;

/// Committed asset-backed facility (warehouse line) against a collateral
/// pool: advance rates, concentration limits and a borrowing-base test drive
/// mandatory repayments, collateral principal recycles while revolving and
/// repays the facility sequentially afterwards, the undrawn commitment
/// accrues a fee, and a term-out ends in a collateral liquidation.
///
/// The engine runs a synthetic two-class structured-credit deal
/// (``synthesized_deal()``); ``project()`` returns the lender's and
/// residual's flows. Rates are decimals or basis points (``*_bp``),
/// percentages are percent values.
///
/// Examples
/// --------
/// >>> from finstack_quant.valuations.instruments import AssetBackedFacility
/// >>> facility = AssetBackedFacility.example()
/// >>> facility.id, facility.drawn.amount, facility.commitment.amount
/// ('ABF-EXAMPLE', 70000000.0, 80000000.0)
/// >>> facility.borrowing_base()["borrowing_base"]["currency"]
/// 'USD'
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "AssetBackedFacility",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyAssetBackedFacility {
    pub(crate) inner: AssetBackedFacility,
}

impl PyAssetBackedFacility {
    pub(crate) fn envelope_json(&self) -> PyResult<String> {
        serialize_typed_instrument_json(
            InstrumentJson::AssetBackedFacility(Box::new(self.inner.clone())),
            "AssetBackedFacility",
        )
    }
}

#[pymethods]
impl PyAssetBackedFacility {
    /// Create a fluent builder (mirrors Rust ``AssetBackedFacility::builder()``).
    ///
    /// Returns
    /// -------
    /// AssetBackedFacilityBuilder
    ///     A builder with fluent, consuming setter methods.
    ///
    /// Examples
    /// --------
    /// >>> from finstack_quant.valuations.instruments import AssetBackedFacility
    /// >>> builder = AssetBackedFacility.builder()
    /// >>> builder.id("WH-1") is builder
    /// True
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn builder() -> PyAssetBackedFacilityBuilder {
        PyAssetBackedFacilityBuilder {
            inner: Some(AssetBackedFacility::builder()),
            credit_model: None,
        }
    }

    /// The canonical example: the example CLO pool financed by a USD 80M
    /// commitment drawn USD 70M at a fixed 6%, 80% advance rate, 20% obligor
    /// limit, two-year revolving period and a 24-month term-out.
    ///
    /// Returns
    /// -------
    /// AssetBackedFacility
    ///     The example facility.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If construction fails (should not occur).
    ///
    /// Examples
    /// --------
    /// >>> from finstack_quant.valuations.instruments import AssetBackedFacility
    /// >>> AssetBackedFacility.example().margin_bp
    /// 600.0
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn example() -> PyResult<Self> {
        AssetBackedFacility::example()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    /// Deserialize from a canonical ``finstack_quant.instrument/1`` envelope.
    ///
    /// Parameters
    /// ----------
    /// json : str
    ///     Envelope JSON whose instrument type is ``asset_backed_facility``.
    ///
    /// Returns
    /// -------
    /// AssetBackedFacility
    ///     The decoded facility.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the JSON is malformed, carries another instrument type or
    ///     fails validation.
    ///
    /// Examples
    /// --------
    /// >>> from finstack_quant.valuations.instruments import AssetBackedFacility
    /// >>> facility = AssetBackedFacility.example()
    /// >>> AssetBackedFacility.from_json(facility.to_json()).id
    /// 'ABF-EXAMPLE'
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        match parse_typed_instrument_json(json)? {
            InstrumentJson::AssetBackedFacility(inner) => Ok(Self { inner: *inner }),
            _ => Err(value_error(
                "expected instrument type \"asset_backed_facility\", got a different instrument type",
            )),
        }
    }

    /// Serialize to the canonical instrument envelope.
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

    /// Return the facility as a plain ``dict`` (canonical serde shape).
    ///
    /// Returns
    /// -------
    /// dict[str, Any]
    ///     Serde form of the Rust facility.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the value cannot be serialized.
    #[pyo3(text_signature = "($self)")]
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner)
    }

    /// Support ``pickle`` through the ``to_json`` / ``from_json`` round-trip.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Price the lender's projected flows (interest, principal and unused
    /// fees) through the shared pricing pipeline.
    ///
    /// Parameters
    /// ----------
    /// market : MarketContext
    ///     Discount and index curves plus fixings for the note and the
    ///     collateral.
    /// as_of : datetime.date
    ///     Valuation date.
    /// model : str, optional
    ///     Pricing model key; ``"default"`` selects discounting.
    /// metrics : list[str], optional
    ///     Metric identifiers to compute alongside the value (for example
    ///     ``"abf_borrowing_base_cushion"``, ``"abf_facility_irr"``,
    ///     ``"dv01"``).
    /// pricing_options : dict | str, optional
    ///     ``PricingOptions`` overrides.
    /// market_history : str, optional
    ///     ``MarketHistory`` JSON for metrics that need past observations.
    ///
    /// Returns
    /// -------
    /// ValuationResult
    ///     Value plus the requested metrics.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the facility fails validation, a curve is missing, or a metric
    ///     cannot be computed.
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

    /// Compute one metric through the shared pricing pipeline.
    ///
    /// Parameters
    /// ----------
    /// market : MarketContext
    ///     Curves and fixings for the projection.
    /// as_of : datetime.date
    ///     Valuation date.
    /// metric_id : str
    ///     Metric identifier (``"abf_borrowing_base"``,
    ///     ``"abf_borrowing_base_cushion"``, ``"abf_advance_rate_utilization"``,
    ///     ``"abf_facility_irr"``, ``"abf_residual_irr"``, ``"dv01"``,
    ///     ``"cs01"`` ...).
    /// model : str, optional
    ///     Pricing model key; ``"default"`` selects discounting.
    ///
    /// Returns
    /// -------
    /// float
    ///     The metric value.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the metric is unknown for this instrument or cannot be computed.
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

    /// Borrowing base on the closing collateral.
    ///
    /// Returns
    /// -------
    /// dict[str, Any]
    ///     ``BorrowingBaseReport`` serde dict: ``eligible_collateral``,
    ///     ``concentration_excess`` and ``borrowing_base`` Money values.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the borrowing-base rules are malformed.
    #[pyo3(text_signature = "($self)")]
    fn borrowing_base<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let report = self.inner.borrowing_base().map_err(core_to_py)?;
        serde_to_py(py, &report)
    }

    /// The two-class structured-credit deal the engine runs for this
    /// facility (facility note + residual, borrowing-base test, reinvestment
    /// window, early-amortization rules and the term-out call).
    ///
    /// Returns
    /// -------
    /// StructuredCredit
    ///     The synthetic deal.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the facility or the synthetic deal fails validation.
    #[pyo3(text_signature = "($self)")]
    fn synthesized_deal(&self) -> PyResult<PyStructuredCredit> {
        let inner = self.inner.synthesized_deal().map_err(core_to_py)?;
        Ok(PyStructuredCredit { inner })
    }

    /// Project the facility through the engine.
    ///
    /// Parameters
    /// ----------
    /// market : MarketContext
    ///     Curves and fixings for the note and the collateral.
    /// as_of : datetime.date
    ///     Valuation date the projection starts from.
    ///
    /// Returns
    /// -------
    /// FacilityProjection
    ///     Facility and residual flows, unused fees and the period record.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the facility fails validation, ``as_of`` is invalid or market
    ///     data is missing.
    #[pyo3(text_signature = "($self, market, as_of)")]
    fn project(
        &self,
        py: Python<'_>,
        market: &Bound<'_, PyAny>,
        as_of: &Bound<'_, PyAny>,
    ) -> PyResult<PyFacilityProjection> {
        let market = extract_market(py, market)?;
        let as_of = extract_date(as_of)?;
        let facility = self.inner.clone();
        let inner = py
            .detach(move || facility.project(&market, as_of))
            .map_err(core_to_py)?;
        Ok(PyFacilityProjection { inner })
    }

    /// Lender IRR: XIRR of ``-drawn`` on ``as_of`` against every projected
    /// interest, principal and fee receipt, as an annual decimal.
    ///
    /// Parameters
    /// ----------
    /// market : MarketContext
    ///     Curves and fixings for the projection.
    /// as_of : datetime.date
    ///     Valuation date the investment is dated on.
    ///
    /// Returns
    /// -------
    /// float
    ///     The internal rate of return.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the projection fails or no rate solves.
    #[pyo3(text_signature = "($self, market, as_of)")]
    fn facility_irr(
        &self,
        py: Python<'_>,
        market: &Bound<'_, PyAny>,
        as_of: &Bound<'_, PyAny>,
    ) -> PyResult<f64> {
        let market = extract_market(py, market)?;
        let as_of = extract_date(as_of)?;
        self.inner.facility_irr(&market, as_of).map_err(core_to_py)
    }

    /// Instrument identifier.
    #[getter]
    fn id(&self) -> String {
        self.inner.id.to_string()
    }

    /// Collateral pool the facility lends against.
    #[getter]
    fn collateral(&self) -> PyAssetPool {
        PyAssetPool {
            inner: self.inner.collateral.clone(),
        }
    }

    /// ``BorrowingBaseRules`` serde dict (``advance_rates`` and
    /// ``concentration_limits``).
    #[getter]
    fn borrowing_base_rules<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.borrowing_base_rules)
    }

    /// Total commitment.
    #[getter]
    fn commitment(&self) -> PyMoney {
        money_to_py(self.inner.commitment)
    }

    /// Amount drawn at closing.
    #[getter]
    fn drawn(&self) -> PyMoney {
        money_to_py(self.inner.drawn)
    }

    /// Undrawn commitment at closing.
    #[getter]
    fn undrawn(&self) -> PyResult<PyMoney> {
        self.inner.undrawn().map(money_to_py).map_err(core_to_py)
    }

    /// Floating index curve identifier, or ``None`` for a fixed all-in rate.
    #[getter]
    fn index_id(&self) -> Option<String> {
        self.inner.index_id.as_ref().map(|id| id.to_string())
    }

    /// Margin over the index (or the all-in fixed rate) in basis points.
    #[getter]
    fn margin_bp(&self) -> f64 {
        self.inner.margin_bp
    }

    /// Fee on the undrawn commitment in basis points per annum.
    #[getter]
    fn unused_fee_bp(&self) -> f64 {
        self.inner.unused_fee_bp
    }

    /// Closing date as ``datetime.date``.
    #[getter]
    fn closing_date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.closing_date)
    }

    /// Scheduled end of the revolving period as ``datetime.date``.
    #[getter]
    fn revolving_end<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.revolving_end)
    }

    /// Effective revolving end (the scheduled end or the earliest dated
    /// amortization event) as ``datetime.date``.
    #[getter]
    fn effective_revolving_end<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.effective_revolving_end())
    }

    /// Final repayment date (maturity, or the revolving end plus the term-out
    /// window) as ``datetime.date``.
    #[getter]
    fn repayment_date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.repayment_date())
    }

    /// Legal final maturity as ``datetime.date``.
    #[getter]
    fn maturity<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.maturity)
    }

    /// Payment frequency.
    #[getter]
    fn frequency(&self) -> PyTenor {
        PyTenor {
            inner: self.inner.frequency,
        }
    }

    /// Accrual day count of the facility interest and unused fee.
    #[getter]
    fn day_count(&self) -> PyDayCount {
        PyDayCount {
            inner: self.inner.day_count,
        }
    }

    /// Payment calendar identifier, or ``None``.
    #[getter]
    fn payment_calendar_id(&self) -> Option<String> {
        self.inner.payment_calendar_id.clone()
    }

    /// Amortization events as ``AmortizationEvent`` serde dicts (``kind`` =
    /// ``"date"`` / ``"cumulative_loss"`` / ``"excess_spread"``).
    #[getter]
    fn amortization_events<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.amortization_events)
    }

    /// Term-out window as its serde dict (``{"months": ...}``), or ``None``.
    #[getter]
    fn term_out<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .term_out
            .as_ref()
            .map(|spec| serde_to_py(py, spec))
            .transpose()
    }

    /// Collateral liquidation price at the term-out end in percent of par,
    /// or ``None`` for par.
    #[getter]
    fn liquidation_price_pct(&self) -> Option<f64> {
        self.inner.liquidation_price_pct
    }

    /// Transaction fees paid ahead of the facility's interest as the
    /// ``DealFees`` serde dict, or ``None``.
    #[getter]
    fn fees<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .fees
            .as_ref()
            .map(|fees| serde_to_py(py, fees))
            .transpose()
    }

    /// Scheduled draws as a list of ``{"date": ..., "amount": Money}`` dicts.
    #[getter]
    fn draw_schedule<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.draw_schedule)
    }

    /// Whether the line is re-advanced up to the borrowing base each
    /// revolving period.
    #[getter]
    fn readvance_to_borrowing_base(&self) -> bool {
        self.inner.readvance_to_borrowing_base
    }

    /// Discount curve identifier.
    #[getter]
    fn discount_curve_id(&self) -> String {
        self.inner.discount_curve_id.to_string()
    }

    /// Collateral behavior as its ``CreditModelConfig`` serde ``dict``.
    #[getter]
    fn credit_model<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.credit_model)
    }

    /// Deterministic prepayment model as a typed ``PrepaymentModelSpec``.
    #[getter]
    fn prepayment_spec(&self) -> PyPrepaymentModelSpec {
        PyPrepaymentModelSpec {
            inner: self.inner.credit_model.prepayment_spec.clone(),
        }
    }

    /// Deterministic default model as a typed ``DefaultModelSpec``.
    #[getter]
    fn default_spec(&self) -> PyDefaultModelSpec {
        PyDefaultModelSpec {
            inner: self.inner.credit_model.default_spec.clone(),
        }
    }

    /// Recovery model as a typed ``RecoveryModelSpec``.
    #[getter]
    fn recovery_spec(&self) -> PyRecoveryModelSpec {
        PyRecoveryModelSpec {
            inner: self.inner.credit_model.recovery_spec.clone(),
        }
    }

    /// Free-form attributes (tags and metadata).
    #[getter]
    fn attributes(&self) -> PyAttributes {
        attributes_to_py(&self.inner.attributes)
    }

    /// Default pricing model key (``"discounting"``).
    #[getter]
    fn default_model(&self) -> String {
        instrument_default_model(&self.inner)
    }

    /// Curves, fixings and series the facility needs from the market.
    ///
    /// Returns
    /// -------
    /// dict[str, Any]
    ///     ``MarketDependencies`` serde dict.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the collateral cannot be normalized.
    #[pyo3(text_signature = "($self)")]
    fn market_dependencies<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        instrument_market_dependencies(py, &self.inner)
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "AssetBackedFacility(id={:?}, commitment={}, drawn={}, margin_bp={}, revolving_end={})",
            self.inner.id.as_str(),
            self.inner.commitment.amount(),
            self.inner.drawn.amount(),
            self.inner.margin_bp,
            self.inner.revolving_end
        )
    }
}

/// Facility and residual projection (``AssetBackedFacility.project``'s
/// return value): the lender's interest and principal, the unused-commitment
/// fees, the residual's flows and the synthetic deal's period record.
///
/// Examples
/// --------
/// >>> import datetime
/// >>> from finstack_quant.core.market_data import DiscountCurve, MarketContext
/// >>> from finstack_quant.valuations.instruments import AssetBackedFacility
/// >>> facility = AssetBackedFacility.example()
/// >>> as_of = facility.closing_date
/// >>> market = MarketContext().insert(DiscountCurve.flat("USD-OIS", as_of, 0.04))
/// >>> projection = facility.project(market, as_of)
/// >>> list(projection.to_dataframe().columns)
/// ['date', 'interest', 'principal', 'unused_fee', 'lender_total', 'residual']
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "FacilityProjection",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyFacilityProjection {
    pub(crate) inner: FacilityProjection,
}

#[pymethods]
impl PyFacilityProjection {
    /// Deserialize from the JSON produced by ``to_json``.
    ///
    /// Parameters
    /// ----------
    /// json : str
    ///     JSON-encoded ``FacilityProjection``.
    ///
    /// Returns
    /// -------
    /// FacilityProjection
    ///     The decoded projection.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``json`` is malformed or carries unknown fields.
    ///
    /// Examples
    /// --------
    /// >>> from finstack_quant.valuations.instruments import FacilityProjection
    /// >>> try:
    /// ...     FacilityProjection.from_json("{}")
    /// ... except ValueError:
    /// ...     print("rejected")
    /// rejected
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner = serde_json::from_str(json)
            .map_err(|err| serde_json_to_py(err, "invalid FacilityProjection JSON"))?;
        Ok(Self { inner })
    }

    /// Serialize to the JSON shape ``from_json`` accepts.
    ///
    /// Returns
    /// -------
    /// str
    ///     JSON-encoded projection.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the value cannot be serialized.
    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Return every field as a plain ``dict`` (canonical serde shape).
    ///
    /// Returns
    /// -------
    /// dict[str, Any]
    ///     Serde form of the projection.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the value cannot be serialized.
    #[pyo3(text_signature = "($self)")]
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner)
    }

    /// Support ``pickle`` through the ``to_json`` / ``from_json`` round-trip.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Interest and principal paid to the facility note.
    #[getter]
    fn facility(&self) -> PyTrancheCashflows {
        PyTrancheCashflows {
            inner: self.inner.facility.clone(),
        }
    }

    /// Cash paid to the residual class.
    #[getter]
    fn residual(&self) -> PyTrancheCashflows {
        PyTrancheCashflows {
            inner: self.inner.residual.clone(),
        }
    }

    /// Unused-commitment fee per payment date as ``(datetime.date, Money)``
    /// pairs.
    #[getter]
    fn unused_fees<'py>(&self, py: Python<'py>) -> PyResult<Vec<(Bound<'py, PyAny>, PyMoney)>> {
        self.inner
            .unused_fees
            .iter()
            .map(|(date, amount)| Ok((date_to_py(py, *date)?, money_to_py(*amount))))
            .collect()
    }

    /// Lender draws applied (scheduled draws and re-advances) per payment
    /// date as ``(datetime.date, Money)`` pairs; outflows in the lender's
    /// cashflows.
    #[getter]
    fn draws<'py>(&self, py: Python<'py>) -> PyResult<Vec<(Bound<'py, PyAny>, PyMoney)>> {
        self.inner
            .draws
            .iter()
            .map(|(date, amount)| Ok((date_to_py(py, *date)?, money_to_py(*amount))))
            .collect()
    }

    /// Every cashflow to the lender (interest, principal and fees, less
    /// draws) per date, as ``(datetime.date, Money)`` pairs.
    #[getter]
    fn lender_cashflows<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<Vec<(Bound<'py, PyAny>, PyMoney)>> {
        self.inner
            .lender_cashflows()
            .into_iter()
            .map(|(date, amount)| Ok((date_to_py(py, date)?, money_to_py(amount))))
            .collect()
    }

    /// Per-period record of the synthetic deal.
    #[getter]
    fn diagnostics(&self) -> PySimulationDiagnostics {
        PySimulationDiagnostics {
            inner: self.inner.diagnostics.clone(),
        }
    }

    /// One row per payment date as a pandas ``DataFrame``.
    ///
    /// Columns: ``date`` (ISO 8601 string), ``interest``, ``principal``,
    /// ``unused_fee``, ``lender_total`` (the three summed) and ``residual``
    /// (cash to the residual class), all in currency units.
    ///
    /// Returns
    /// -------
    /// pandas.DataFrame
    ///     The projection in date order.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the rows cannot be serialized.
    #[pyo3(text_signature = "($self)")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        use std::collections::BTreeMap;
        let mut by_date: BTreeMap<finstack_quant_core::dates::Date, [f64; 5]> = BTreeMap::new();
        for (index, flows) in [
            &self.inner.facility.interest_flows,
            &self.inner.facility.principal_flows,
            &self.inner.unused_fees,
            &self.inner.residual.cashflows,
            &self.inner.draws,
        ]
        .into_iter()
        .enumerate()
        {
            for (date, amount) in flows {
                by_date.entry(*date).or_default()[index] += amount.amount();
            }
        }
        let rows: Vec<serde_json::Value> = by_date
            .iter()
            .map(|(date, values)| {
                serde_json::json!({
                    "date": date.to_string(),
                    "interest": values[0],
                    "principal": values[1],
                    "unused_fee": values[2],
                    "draw": values[4],
                    "lender_total": values[0] + values[1] + values[2] - values[4],
                    "residual": values[3],
                })
            })
            .collect();
        serde_rows_to_dataframe_with_schema(
            py,
            &rows,
            &[
                ("date", "str"),
                ("interest", "float64"),
                ("principal", "float64"),
                ("unused_fee", "float64"),
                ("draw", "float64"),
                ("lender_total", "float64"),
                ("residual", "float64"),
            ],
        )
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "FacilityProjection(payments={}, total_interest={}, total_principal={}, unused_fees={})",
            self.inner.facility.cashflows.len(),
            self.inner.facility.total_interest.amount(),
            self.inner.facility.total_principal.amount(),
            self.inner.unused_fees.len()
        )
    }
}

/// Fluent builder for [`PyAssetBackedFacility`]; wraps the Rust
/// `FinancialBuilder`-generated builder (consuming setters).
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "AssetBackedFacilityBuilder",
    skip_from_py_object
)]
pub struct PyAssetBackedFacilityBuilder {
    inner: Option<FacilityBuilderInner>,
    /// Collateral behavior assembled by the per-field setters; applied on ``build``.
    credit_model: Option<CreditModelConfig>,
}

fn take_facility(b: &mut PyAssetBackedFacilityBuilder) -> PyResult<FacilityBuilderInner> {
    b.inner
        .take()
        .ok_or_else(|| value_error("builder already consumed by build()"))
}

#[pymethods]
impl PyAssetBackedFacilityBuilder {
    /// Set the facility identifier.
    ///
    /// Parameters
    /// ----------
    /// value : str
    ///     Stable facility identifier.
    ///
    /// Returns
    /// -------
    /// AssetBackedFacilityBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If this builder was already consumed by ``build``.
    #[pyo3(text_signature = "($self, value)")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let converted = InstrumentId::new(value.to_string());
        let b = take_facility(&mut slf)?;
        slf.inner = Some(b.id(converted));
        Ok(slf)
    }

    /// Set the collateral pool.
    ///
    /// Parameters
    /// ----------
    /// value : AssetPool
    ///     Collateral pool (asset rows, rep lines or instrument collateral).
    ///
    /// Returns
    /// -------
    /// AssetBackedFacilityBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If this builder was already consumed by ``build``.
    #[pyo3(text_signature = "($self, value)")]
    fn collateral<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: PyRef<'_, PyAssetPool>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let b = take_facility(&mut slf)?;
        slf.inner = Some(b.collateral(value.inner.clone()));
        Ok(slf)
    }

    /// Set the advance rates, eligibility and concentration limits.
    ///
    /// Parameters
    /// ----------
    /// value : dict | str
    ///     ``BorrowingBaseRules`` serde object: ``advance_rates`` (each
    ///     ``asset_class`` wire name or ``"*"``, decimal ``rate`` and an
    ///     optional ``eligibility``) and ``concentration_limits`` (``scope``
    ///     ``"obligor"`` / ``"industry"`` / ``"asset_class"``, ``max_pct``).
    ///
    /// Returns
    /// -------
    /// AssetBackedFacilityBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` does not match the ``BorrowingBaseRules`` shape or
    ///     this builder was already consumed by ``build``.
    #[pyo3(text_signature = "($self, value)")]
    fn borrowing_base_rules<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let rules: BorrowingBaseRules =
            crate::bindings::module_utils::py_to_serde(py, value, "borrowing_base_rules")?;
        let b = take_facility(&mut slf)?;
        slf.inner = Some(b.borrowing_base_rules(rules));
        Ok(slf)
    }

    /// Set the total commitment.
    ///
    /// Parameters
    /// ----------
    /// value : Money | float
    ///     Commitment; a bare amount is tagged with ``currency``.
    /// currency : str, optional
    ///     ISO-4217 code for a bare amount (ignored for ``Money``).
    ///
    /// Returns
    /// -------
    /// AssetBackedFacilityBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a bare amount has no currency or this builder was already
    ///     consumed by ``build``.
    /// TypeError
    ///     If ``value`` is neither ``Money`` nor a number.
    #[pyo3(signature = (value, currency=None))]
    #[pyo3(text_signature = "($self, value, currency=None)")]
    fn commitment<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
        currency: Option<&str>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let money = money_from_py(value, currency, "commitment")?;
        let b = take_facility(&mut slf)?;
        slf.inner = Some(b.commitment(money));
        Ok(slf)
    }

    /// Set the amount drawn at closing.
    ///
    /// Parameters
    /// ----------
    /// value : Money | float
    ///     Drawn balance (at most the commitment and below the collateral);
    ///     a bare amount is tagged with ``currency``.
    /// currency : str, optional
    ///     ISO-4217 code for a bare amount (ignored for ``Money``).
    ///
    /// Returns
    /// -------
    /// AssetBackedFacilityBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a bare amount has no currency or this builder was already
    ///     consumed by ``build``.
    /// TypeError
    ///     If ``value`` is neither ``Money`` nor a number.
    #[pyo3(signature = (value, currency=None))]
    #[pyo3(text_signature = "($self, value, currency=None)")]
    fn drawn<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
        currency: Option<&str>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let money = money_from_py(value, currency, "drawn")?;
        let b = take_facility(&mut slf)?;
        slf.inner = Some(b.drawn(money));
        Ok(slf)
    }

    /// Set the floating index; omit for a fixed all-in rate.
    ///
    /// Parameters
    /// ----------
    /// value : str
    ///     Forward-curve identifier (e.g. ``"USD-SOFR-3M"``).
    ///
    /// Returns
    /// -------
    /// AssetBackedFacilityBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If this builder was already consumed by ``build``.
    #[pyo3(text_signature = "($self, value)")]
    fn index_id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let converted = CurveId::new(value.to_string());
        let b = take_facility(&mut slf)?;
        slf.inner = Some(b.index_id(converted));
        Ok(slf)
    }

    /// Set the margin over the index (or the all-in fixed rate).
    ///
    /// Parameters
    /// ----------
    /// value : float
    ///     Basis points per annum.
    ///
    /// Returns
    /// -------
    /// AssetBackedFacilityBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If this builder was already consumed by ``build``.
    #[pyo3(text_signature = "($self, value)")]
    fn margin_bp<'py>(mut slf: PyRefMut<'py, Self>, value: f64) -> PyResult<PyRefMut<'py, Self>> {
        let converted = value;
        let b = take_facility(&mut slf)?;
        slf.inner = Some(b.margin_bp(converted));
        Ok(slf)
    }

    /// Set the fee on the undrawn commitment.
    ///
    /// Parameters
    /// ----------
    /// value : float
    ///     Basis points per annum.
    ///
    /// Returns
    /// -------
    /// AssetBackedFacilityBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If this builder was already consumed by ``build``.
    #[pyo3(text_signature = "($self, value)")]
    fn unused_fee_bp<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: f64,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let converted = value;
        let b = take_facility(&mut slf)?;
        slf.inner = Some(b.unused_fee_bp(converted));
        Ok(slf)
    }

    /// Set the closing date.
    ///
    /// Parameters
    /// ----------
    /// value : datetime.date
    ///     Closing date; the first payment date is one frequency later.
    ///
    /// Returns
    /// -------
    /// AssetBackedFacilityBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the date is invalid or this builder was already consumed.
    #[pyo3(text_signature = "($self, value)")]
    fn closing_date<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let date = extract_date(value)?;
        let b = take_facility(&mut slf)?;
        slf.inner = Some(b.closing_date(date));
        Ok(slf)
    }

    /// Set the scheduled end of the revolving period.
    ///
    /// Parameters
    /// ----------
    /// value : datetime.date
    ///     Revolving end; after it, collateral principal repays the facility.
    ///
    /// Returns
    /// -------
    /// AssetBackedFacilityBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the date is invalid or this builder was already consumed.
    #[pyo3(text_signature = "($self, value)")]
    fn revolving_end<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let date = extract_date(value)?;
        let b = take_facility(&mut slf)?;
        slf.inner = Some(b.revolving_end(date));
        Ok(slf)
    }

    /// Set the legal final maturity.
    ///
    /// Parameters
    /// ----------
    /// value : datetime.date
    ///     Legal final maturity.
    ///
    /// Returns
    /// -------
    /// AssetBackedFacilityBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the date is invalid or this builder was already consumed.
    #[pyo3(text_signature = "($self, value)")]
    fn maturity<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let date = extract_date(value)?;
        let b = take_facility(&mut slf)?;
        slf.inner = Some(b.maturity(date));
        Ok(slf)
    }

    /// Set the payment frequency.
    ///
    /// Parameters
    /// ----------
    /// value : Tenor | str
    ///     Payment frequency of interest, fees and the borrowing-base test
    ///     (``Tenor`` or a string such as ``"3M"``).
    ///
    /// Returns
    /// -------
    /// AssetBackedFacilityBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the string is not a tenor or this builder was already consumed
    ///     by ``build``.
    #[pyo3(text_signature = "($self, value)")]
    fn frequency<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let tenor = tenor_from_py(value, "frequency")?;
        let b = take_facility(&mut slf)?;
        slf.inner = Some(b.frequency(tenor));
        Ok(slf)
    }

    /// Set the accrual day count.
    ///
    /// Parameters
    /// ----------
    /// value : DayCount | str
    ///     Accrual convention (``DayCount`` or a name such as ``"act_360"``);
    ///     Act/360 when never set.
    ///
    /// Returns
    /// -------
    /// AssetBackedFacilityBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the string is not a day count or this builder was already
    ///     consumed by ``build``.
    #[pyo3(text_signature = "($self, value)")]
    fn day_count<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let day_count = day_count_from_py(value, "day_count")?;
        let b = take_facility(&mut slf)?;
        slf.inner = Some(b.day_count(day_count));
        Ok(slf)
    }

    /// Set the payment calendar.
    ///
    /// Parameters
    /// ----------
    /// value : str
    ///     Holiday calendar identifier (e.g. ``"nyse"``); required for
    ///     pricing.
    ///
    /// Returns
    /// -------
    /// AssetBackedFacilityBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If this builder was already consumed by ``build``.
    #[pyo3(text_signature = "($self, value)")]
    fn payment_calendar_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let converted = value.to_string();
        let b = take_facility(&mut slf)?;
        slf.inner = Some(b.payment_calendar_id(converted));
        Ok(slf)
    }

    /// Set the events that end revolving early.
    ///
    /// Parameters
    /// ----------
    /// value : list[dict] | str
    ///     ``AmortizationEvent`` objects: ``{"kind": "date", "date": ...}``,
    ///     ``{"kind": "cumulative_loss", "max_pct": ...}`` or
    ///     ``{"kind": "excess_spread", "min_3m": ...}``.
    ///
    /// Returns
    /// -------
    /// AssetBackedFacilityBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` does not match the event shape or this builder was
    ///     already consumed by ``build``.
    #[pyo3(text_signature = "($self, value)")]
    fn amortization_events<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let events: Vec<AmortizationEvent> =
            crate::bindings::module_utils::py_to_serde(py, value, "amortization_events")?;
        let b = take_facility(&mut slf)?;
        slf.inner = Some(b.amortization_events(events));
        Ok(slf)
    }

    /// Set the term-out window after revolving.
    ///
    /// Parameters
    /// ----------
    /// value : int
    ///     Months after the revolving end at which the remaining
    ///     collateral is liquidated to repay the facility.
    ///
    /// Returns
    /// -------
    /// AssetBackedFacilityBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If this builder was already consumed by ``build``.
    #[pyo3(text_signature = "($self, value)")]
    fn term_out<'py>(mut slf: PyRefMut<'py, Self>, value: u32) -> PyResult<PyRefMut<'py, Self>> {
        let converted = TermOutSpec { months: value };
        let b = take_facility(&mut slf)?;
        slf.inner = Some(b.term_out(converted));
        Ok(slf)
    }

    /// Set the transaction fees paid through the waterfall ahead of the
    /// facility's interest.
    ///
    /// Parameters
    /// ----------
    /// value : dict | str
    ///     ``DealFees`` serde object (``trustee_fee_annual`` Money,
    ///     ``senior_mgmt_fee_bp``, ``subordinated_mgmt_fee_bp``,
    ///     ``servicing_fee_bp``, optional ``master_servicer_fee_bp`` /
    ///     ``special_servicer_fee_bp`` / ``incentive_fee``).
    ///
    /// Returns
    /// -------
    /// AssetBackedFacilityBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` does not match the ``DealFees`` shape or this builder
    ///     was already consumed by ``build``.
    #[pyo3(text_signature = "($self, value)")]
    fn fees<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let fees: DealFees = crate::bindings::module_utils::py_to_serde(py, value, "fees")?;
        let b = take_facility(&mut slf)?;
        slf.inner = Some(b.fees(fees));
        Ok(slf)
    }

    /// Set the scheduled draws after closing.
    ///
    /// Parameters
    /// ----------
    /// value : list[dict] | str
    ///     ``FacilityDraw`` serde objects ``{"date": "2025-01-01", "amount":
    ///     {"amount": 10000000.0, "currency": "USD"}}``, ascending by date;
    ///     each is applied on the first payment date at or after its date.
    ///
    /// Returns
    /// -------
    /// AssetBackedFacilityBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` does not match the ``FacilityDraw`` shape or this
    ///     builder was already consumed by ``build``.
    #[pyo3(text_signature = "($self, value)")]
    fn draw_schedule<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let draws: Vec<FacilityDraw> =
            crate::bindings::module_utils::py_to_serde(py, value, "draw_schedule")?;
        let b = take_facility(&mut slf)?;
        slf.inner = Some(b.draw_schedule(draws));
        Ok(slf)
    }

    /// Set whether the line is re-advanced up to the borrowing base each
    /// revolving period.
    ///
    /// Parameters
    /// ----------
    /// value : bool
    ///     ``True`` draws ``min(commitment, borrowing base) − balance`` every
    ///     revolving period.
    ///
    /// Returns
    /// -------
    /// AssetBackedFacilityBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If this builder was already consumed by ``build``.
    #[pyo3(text_signature = "($self, value)")]
    fn readvance_to_borrowing_base<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: bool,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let b = take_facility(&mut slf)?;
        slf.inner = Some(b.readvance_to_borrowing_base(value));
        Ok(slf)
    }

    /// Set the collateral liquidation price at the term-out end.
    ///
    /// Parameters
    /// ----------
    /// value : float
    ///     Percent of par (``100.0`` = par).
    ///
    /// Returns
    /// -------
    /// AssetBackedFacilityBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If this builder was already consumed by ``build``.
    #[pyo3(text_signature = "($self, value)")]
    fn liquidation_price_pct<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: f64,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let converted = value;
        let b = take_facility(&mut slf)?;
        slf.inner = Some(b.liquidation_price_pct(converted));
        Ok(slf)
    }

    /// Set the discount curve.
    ///
    /// Parameters
    /// ----------
    /// value : str
    ///     Discount curve identifier.
    ///
    /// Returns
    /// -------
    /// AssetBackedFacilityBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If this builder was already consumed by ``build``.
    #[pyo3(text_signature = "($self, value)")]
    fn discount_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let converted = CurveId::new(value.to_string());
        let b = take_facility(&mut slf)?;
        slf.inner = Some(b.discount_curve_id(converted));
        Ok(slf)
    }

    /// Replace the whole collateral behavior model.
    ///
    /// Parameters
    /// ----------
    /// value : dict | str
    ///     ``CreditModelConfig`` serde object; the per-field setters
    ///     (:meth:`prepayment_spec` ...) then modify it.
    ///
    /// Returns
    /// -------
    /// AssetBackedFacilityBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` does not match the shape or this builder was already
    ///     consumed by ``build``.
    #[pyo3(text_signature = "($self, value)")]
    fn credit_model<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let model: CreditModelConfig =
            crate::bindings::module_utils::py_to_serde(py, value, "credit_model")?;
        if slf.inner.is_none() {
            return Err(value_error("builder already consumed by build()"));
        }
        slf.credit_model = Some(model);
        Ok(slf)
    }

    /// Set the deterministic prepayment model of the collateral.
    ///
    /// Parameters
    /// ----------
    /// value : PrepaymentModelSpec | dict | str
    ///     Typed spec or its serde form.
    ///
    /// Returns
    /// -------
    /// AssetBackedFacilityBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` does not match the shape or this builder was already
    ///     consumed by ``build``.
    #[pyo3(text_signature = "($self, value)")]
    fn prepayment_spec<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let spec: PrepaymentModelSpec = if let Ok(typed) = value.cast::<PyPrepaymentModelSpec>() {
            typed.borrow().inner.clone()
        } else {
            crate::bindings::module_utils::py_to_serde(py, value, "prepayment_spec")?
        };
        if slf.inner.is_none() {
            return Err(value_error("builder already consumed by build()"));
        }
        slf.credit_model
            .get_or_insert_with(CreditModelConfig::default)
            .prepayment_spec = spec;
        Ok(slf)
    }

    /// Set the deterministic default model of the collateral.
    ///
    /// Parameters
    /// ----------
    /// value : DefaultModelSpec | dict | str
    ///     Typed spec or its serde form.
    ///
    /// Returns
    /// -------
    /// AssetBackedFacilityBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` does not match the shape or this builder was already
    ///     consumed by ``build``.
    #[pyo3(text_signature = "($self, value)")]
    fn default_spec<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let spec: DefaultModelSpec = if let Ok(typed) = value.cast::<PyDefaultModelSpec>() {
            typed.borrow().inner.clone()
        } else {
            crate::bindings::module_utils::py_to_serde(py, value, "default_spec")?
        };
        if slf.inner.is_none() {
            return Err(value_error("builder already consumed by build()"));
        }
        slf.credit_model
            .get_or_insert_with(CreditModelConfig::default)
            .default_spec = spec;
        Ok(slf)
    }

    /// Set the recovery model of the collateral.
    ///
    /// Parameters
    /// ----------
    /// value : RecoveryModelSpec | dict | str
    ///     Typed spec or its serde form.
    ///
    /// Returns
    /// -------
    /// AssetBackedFacilityBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` does not match the shape or this builder was already
    ///     consumed by ``build``.
    #[pyo3(text_signature = "($self, value)")]
    fn recovery_spec<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let spec: RecoveryModelSpec = if let Ok(typed) = value.cast::<PyRecoveryModelSpec>() {
            typed.borrow().inner.clone()
        } else {
            crate::bindings::module_utils::py_to_serde(py, value, "recovery_spec")?
        };
        if slf.inner.is_none() {
            return Err(value_error("builder already consumed by build()"));
        }
        slf.credit_model
            .get_or_insert_with(CreditModelConfig::default)
            .recovery_spec = spec;
        Ok(slf)
    }

    /// Set free-form attributes (tags and metadata).
    ///
    /// Parameters
    /// ----------
    /// value : Attributes | dict[str, str]
    ///     Attribute bag; a dict populates ``meta`` (an optional ``"tags"``
    ///     list populates ``tags``).
    ///
    /// Returns
    /// -------
    /// AssetBackedFacilityBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` is neither ``Attributes`` nor a string dict, or this
    ///     builder was already consumed by ``build``.
    #[pyo3(text_signature = "($self, value)")]
    fn attributes<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let attributes = attributes_from_py(value)?;
        let b = take_facility(&mut slf)?;
        slf.inner = Some(b.attributes(attributes));
        Ok(slf)
    }

    /// Build the validated facility.
    ///
    /// Returns
    /// -------
    /// AssetBackedFacility
    ///     The validated facility.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the builder was already consumed, a required field is missing,
    ///     or the facility fails validation.
    #[pyo3(text_signature = "($self)")]
    fn build(mut slf: PyRefMut<'_, Self>) -> PyResult<PyAssetBackedFacility> {
        let mut b = take_facility(&mut slf)?;
        if let Some(model) = slf.credit_model.take() {
            b = b.credit_model(model);
        }
        let inner = b.build().map_err(core_to_py)?;
        inner.validate().map_err(core_to_py)?;
        Ok(PyAssetBackedFacility { inner })
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "AssetBackedFacilityBuilder(consumed={})",
            bool_repr(self.inner.is_none())
        )
    }
}
