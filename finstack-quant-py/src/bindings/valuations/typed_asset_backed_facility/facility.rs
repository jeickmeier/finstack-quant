//! `AssetBackedFacility`, `AssetBackedFacilityBuilder` and `FacilityProjection`.

use pyo3::prelude::*;

use crate::bindings::cashflows::builder::specs::{
    PyDefaultModelSpec, PyPrepaymentModelSpec, PyRecoveryModelSpec,
};
use crate::bindings::core::dates::daycount::PyDayCount;
use crate::bindings::core::dates::tenor::PyTenor;
use crate::bindings::core::money::PyMoney;
use crate::bindings::date_utils::{date_to_py, py_to_date};
use crate::bindings::extract::extract_market;
use crate::bindings::pandas_utils::serde_to_py;
use crate::bindings::valuations::convert::{
    attributes_from_py, day_count_from_py, money_from_py, money_to_py, tenor_from_py,
};
use crate::bindings::valuations::typed_macros::{
    builder_set, instrument_envelope_methods, instrument_pricing_methods,
};
use crate::bindings::valuations::typed_structured_credit::{
    PyAssetPool, PySimulationDiagnostics, PyStructuredCredit, PyTrancheCashflows,
};
use crate::errors::{core_to_py, display_to_py, serde_json_to_py};
use finstack_quant_cashflows::builder::RateSpec;
use finstack_quant_cashflows::builder::{DefaultModelSpec, PrepaymentModelSpec, RecoveryModelSpec};
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_valuations::instruments::fixed_income::asset_backed_facility::{
    AmortizationEvent, AssetBackedFacility, FacilityProjection, TermOutSpec,
};
use finstack_quant_valuations::instruments::fixed_income::loan_terms::DrawEvent;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::BorrowingBaseRules;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::CreditModelConfig;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::DealFees;

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

instrument_envelope_methods!(
    PyAssetBackedFacility,
    AssetBackedFacility,
    "asset_backed_facility",
    builder = PyAssetBackedFacilityBuilder("AssetBackedFacilityBuilder", no_fields),
    seed = AssetBackedFacility::builder(),
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
    build
);
instrument_pricing_methods!(PyAssetBackedFacility);

#[pymethods]
impl PyAssetBackedFacility {
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
    /// >>> AssetBackedFacility.example().rate
    /// {'fixed': {'rate': 0.06}}
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn example() -> PyResult<Self> {
        AssetBackedFacility::example()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
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
    ///     Facility and residual flows, commitment fees and the period record.
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
        let as_of = py_to_date(as_of)?;
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
        let as_of = py_to_date(as_of)?;
        self.inner.facility_irr(&market, as_of).map_err(core_to_py)
    }

    /// ``BorrowingBaseRules`` serde dict (``advance_rates`` and
    /// ``concentration_limits``).
    #[getter]
    fn borrowing_base_rules<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.borrowing_base_rules)
    }

    /// Facility coupon as its ``RateSpec`` serde ``dict``: ``{"fixed":
    /// {"rate": r}}`` with a decimal all-in rate, or ``{"floating": {...}}``
    /// with a ``FloatingRateSpec``.
    #[getter]
    fn rate<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.rate)
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
    ///
    /// Raises ``ValueError`` if the term-out window exceeds the supported calendar range.
    fn repayment_date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.repayment_date().map_err(core_to_py)?)
    }

    /// Legal final maturity as ``datetime.date``.
    #[getter]
    fn maturity<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.maturity)
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
    fn draws<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.draws)
    }

    /// Collateral behavior as its ``CreditModelConfig`` serde ``dict``.
    #[getter]
    fn credit_model<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.credit_model)
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "AssetBackedFacility(id={:?}, commitment={}, drawn={}, rate={}, revolving_end={})",
            self.inner.id.as_str(),
            self.inner.commitment.amount(),
            self.inner.drawn.amount(),
            serde_json::to_string(&self.inner.rate).unwrap_or_default(),
            self.inner.revolving_end
        )
    }
}

crate::bindings::macros::py_getters!(PyAssetBackedFacility |s| {
    /// Collateral pool the facility lends against.
    collateral -> PyAssetPool = PyAssetPool { inner: s.inner.collateral.clone() };

    /// Total commitment.
    commitment -> PyMoney = money_to_py(s.inner.commitment);

    /// Amount drawn at closing.
    drawn -> PyMoney = money_to_py(s.inner.drawn);

    /// Undrawn commitment at closing.
    undrawn -> PyResult<PyMoney> = s.inner.undrawn().map(money_to_py).map_err(core_to_py);

    /// Commitment fee on the undrawn commitment, in basis points per annum.
    commitment_fee_bp -> f64 =
        rust_decimal::prelude::ToPrimitive::to_f64(&s.inner.commitment_fee_bp).unwrap_or_default();

    /// Payment frequency.
    frequency -> PyTenor = PyTenor { inner: s.inner.frequency };

    /// Accrual day count of the facility interest and commitment fee.
    day_count -> PyDayCount = PyDayCount { inner: s.inner.day_count };

    /// Payment calendar identifier, or ``None``.
    calendar_id -> Option<String> = s.inner.calendar_id.as_ref().map(ToString::to_string);

    /// Collateral liquidation price at the term-out end in percent of par,
    /// or ``None`` for par.
    liquidation_price_pct -> Option<f64> = s.inner.liquidation_price_pct;

    /// Whether the line is re-advanced up to the borrowing base each
    /// revolving period.
    readvance_to_borrowing_base -> bool = s.inner.readvance_to_borrowing_base;

    /// Discount curve identifier.
    discount_curve_id -> String = s.inner.discount_curve_id.to_string();

    /// Deterministic prepayment model as a typed ``PrepaymentModelSpec``.
    prepayment_spec -> PyPrepaymentModelSpec =
        PyPrepaymentModelSpec { inner: s.inner.credit_model.prepayment_spec.clone() };

    /// Deterministic default model as a typed ``DefaultModelSpec``.
    default_spec -> PyDefaultModelSpec =
        PyDefaultModelSpec { inner: s.inner.credit_model.default_spec.clone() };

    /// Recovery model as a typed ``RecoveryModelSpec``.
    recovery_spec -> PyRecoveryModelSpec =
        PyRecoveryModelSpec { inner: s.inner.credit_model.recovery_spec.clone() };
});

/// Facility and residual projection (``AssetBackedFacility.project``'s
/// return value): the lender's interest and principal, the commitment fees,
/// the residual's flows and the synthetic deal's period record.
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
/// ['date', 'interest', 'principal', 'commitment_fee', 'draw', 'lender_total', 'residual']
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

    /// Commitment fee per payment date as ``(datetime.date, Money)`` pairs.
    #[getter]
    fn commitment_fees<'py>(&self, py: Python<'py>) -> PyResult<Vec<(Bound<'py, PyAny>, PyMoney)>> {
        self.inner
            .commitment_fees
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
            .map_err(core_to_py)?
            .into_iter()
            .map(|(date, amount)| Ok((date_to_py(py, date)?, money_to_py(amount))))
            .collect()
    }

    /// One row per payment date as a pandas ``DataFrame``.
    ///
    /// Columns: ``date`` (ISO 8601 string), ``interest``, ``principal``,
    /// ``commitment_fee``, ``draw``, ``lender_total`` and ``residual``
    /// (cash to the residual class), all in currency units. The component
    /// columns pivot the projection's flow lists by date; ``lender_total`` is
    /// the Rust ``lender_cashflows`` amount on that date (the same flows the
    /// ``lender_cashflows`` property and the facility IRR use).
    ///
    /// Returns
    /// -------
    /// pandas.DataFrame
    ///     The projection in date order.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a component carries a different currency from the facility, or the
    ///     rows cannot be serialized.
    #[pyo3(text_signature = "($self)")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::core::table::envelope_to_pandas(
            py,
            &self.inner.to_table().map_err(crate::errors::core_to_py)?,
        )
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "FacilityProjection(payments={}, total_interest={}, total_principal={}, commitment_fees={})",
            self.inner.facility.cashflows.len(),
            self.inner.facility.total_interest.amount(),
            self.inner.facility.total_principal.amount(),
            self.inner.commitment_fees.len()
        )
    }
}

crate::bindings::macros::py_getters!(PyFacilityProjection |s| {
    /// Interest and principal paid to the facility note.
    facility -> PyTrancheCashflows = PyTrancheCashflows { inner: s.inner.facility.clone() };

    /// Cash paid to the residual class.
    residual -> PyTrancheCashflows = PyTrancheCashflows { inner: s.inner.residual.clone() };

    /// Per-period record of the synthetic deal.
    diagnostics -> PySimulationDiagnostics =
        PySimulationDiagnostics { inner: s.inner.diagnostics.clone() };
});

/// Fluent builder for [`PyAssetBackedFacility`]; wraps the Rust
/// `FinancialBuilder`-generated builder (consuming setters).
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "AssetBackedFacilityBuilder",
    skip_from_py_object
)]
pub struct PyAssetBackedFacilityBuilder {
    inner: Option<FacilityBuilderInner>,
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
        builder_set!(slf, |b: FacilityBuilderInner| b.id(converted))
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
        builder_set!(slf, |b: FacilityBuilderInner| b
            .collateral(value.inner.clone()))
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
        builder_set!(slf, |b: FacilityBuilderInner| b.borrowing_base_rules(rules))
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
        builder_set!(slf, |b: FacilityBuilderInner| b.commitment(money))
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
        builder_set!(slf, |b: FacilityBuilderInner| b.drawn(money))
    }

    /// Set the facility coupon.
    ///
    /// Parameters
    /// ----------
    /// value : float | dict | str
    ///     A bare decimal builds a fixed all-in rate (``0.06`` = 6%); a
    ///     ``dict`` or JSON ``str`` in the ``RateSpec`` serde shape
    ///     (``{"fixed": {"rate": 0.06}}`` or ``{"floating": {...}}`` with a
    ///     ``FloatingRateSpec``) is used verbatim.
    ///
    /// Returns
    /// -------
    /// AssetBackedFacilityBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the spec does not match the serde shape or this builder was
    ///     already consumed by ``build``.
    #[pyo3(text_signature = "($self, value)")]
    fn rate<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let spec: RateSpec = if let Ok(rate) = value.extract::<f64>() {
            RateSpec::Fixed { rate }
        } else {
            crate::bindings::module_utils::py_to_serde(py, value, "rate")?
        };
        builder_set!(slf, |b: FacilityBuilderInner| b.rate(spec))
    }

    /// Set the commitment fee on the undrawn commitment.
    ///
    /// Parameters
    /// ----------
    /// value : float
    ///     Basis points per annum (``50.0`` = 0.50%).
    ///
    /// Returns
    /// -------
    /// AssetBackedFacilityBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` is not finite or this builder was already consumed by
    ///     ``build``.
    #[pyo3(text_signature = "($self, value)")]
    fn commitment_fee_bp<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: f64,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let converted =
            crate::bindings::valuations::instruments::decimal_from_f64(value, "commitment_fee_bp")?;
        builder_set!(slf, |b: FacilityBuilderInner| b
            .commitment_fee_bp(converted))
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
        let date = py_to_date(value)?;
        builder_set!(slf, |b: FacilityBuilderInner| b.closing_date(date))
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
        let date = py_to_date(value)?;
        builder_set!(slf, |b: FacilityBuilderInner| b.revolving_end(date))
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
        let date = py_to_date(value)?;
        builder_set!(slf, |b: FacilityBuilderInner| b.maturity(date))
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
        builder_set!(slf, |b: FacilityBuilderInner| b.frequency(tenor))
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
        builder_set!(slf, |b: FacilityBuilderInner| b.day_count(day_count))
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
    fn calendar_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let converted = value.to_string();
        builder_set!(slf, |b: FacilityBuilderInner| b
            .calendar_id(converted.into()))
    }

    /// Set the events that end revolving early.
    ///
    /// Parameters
    /// ----------
    /// value : list[dict] | str
    ///     ``AmortizationEvent`` objects: ``{"kind": "date", "date": ...}``,
    ///     ``{"kind": "cumulative_loss", "max_cumulative_loss": ...}`` or
    ///     ``{"kind": "excess_spread", "min_excess_spread_3m": ...}``.
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
        builder_set!(slf, |b: FacilityBuilderInner| b.amortization_events(events))
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
        builder_set!(slf, |b: FacilityBuilderInner| b.term_out(converted))
    }

    /// Set the transaction fees paid through the waterfall ahead of the
    /// facility's interest.
    ///
    /// Parameters
    /// ----------
    /// value : dict | str
    ///     ``DealFees`` serde object (``trustee_fee`` Money per annum,
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
        builder_set!(slf, |b: FacilityBuilderInner| b.fees(fees))
    }

    /// Set the scheduled draws after closing.
    ///
    /// Parameters
    /// ----------
    /// value : list[dict] | str
    ///     ``DrawEvent`` serde objects ``{"date": "2025-01-01", "amount":
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
    ///     If ``value`` does not match the ``DrawEvent`` shape or this
    ///     builder was already consumed by ``build``.
    #[pyo3(text_signature = "($self, value)")]
    fn draws<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let draws: Vec<DrawEvent> = crate::bindings::module_utils::py_to_serde(py, value, "draws")?;
        builder_set!(slf, |b: FacilityBuilderInner| b.draws(draws))
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
        builder_set!(slf, |b: FacilityBuilderInner| b
            .readvance_to_borrowing_base(value))
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
        builder_set!(slf, |b: FacilityBuilderInner| b
            .liquidation_price_pct(converted))
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
        builder_set!(slf, |b: FacilityBuilderInner| b
            .discount_curve_id(converted))
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
        builder_set!(slf, |b: FacilityBuilderInner| b.credit_model(model))
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
        builder_set!(slf, |b: FacilityBuilderInner| b.prepayment_spec(spec))
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
        builder_set!(slf, |b: FacilityBuilderInner| b.default_spec(spec))
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
        builder_set!(slf, |b: FacilityBuilderInner| b.recovery_spec(spec))
    }

    /// Set free-form attributes (tags and metadata).
    ///
    /// Parameters
    /// ----------
    /// value : Attributes | dict | None
    ///     Attribute bag: an ``Attributes`` or its serde ``dict`` form
    ///     (``{"tags": [...], "meta": {...}}``, as ``to_dict()`` returns it);
    ///     ``None`` clears it.
    ///
    /// Returns
    /// -------
    /// AssetBackedFacilityBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// TypeError
    ///     If ``value`` is neither ``Attributes``, a ``dict`` nor ``None``.
    /// ValueError
    ///     If the ``dict`` has a key other than ``tags`` / ``meta`` or a non-string tag or
    ///     value, or this builder was already consumed by ``build``.
    #[pyo3(text_signature = "($self, value)")]
    fn attributes<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let attributes = attributes_from_py(value)?;
        builder_set!(slf, |b: FacilityBuilderInner| b.attributes(attributes))
    }
}
