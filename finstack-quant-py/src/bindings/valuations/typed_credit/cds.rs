//! Credit default swap Python wrappers and fluent builder.

use pyo3::prelude::*;

use crate::bindings::core::money::PyMoney;
use crate::bindings::date_utils::{date_to_py, extract_date};
use crate::bindings::extract::extract_market;
use crate::errors::{core_to_py, value_error};
use finstack_quant_core::types::InstrumentId;
use finstack_quant_valuations::instruments::credit_derivatives::cds::CdsValuationConvention;
use finstack_quant_valuations::instruments::{Instrument, InstrumentJson};
use finstack_quant_valuations::market::conventions::ids::CdsDocClause;
use finstack_quant_valuations::market::conventions::CdsConvention;
use rust_decimal::prelude::ToPrimitive;

use super::super::convert::{
    attributes_from_py, builder_repr, date_repr, dated_money_from_py, enum_to_py_string,
    money_repr, money_to_py, opt_repr,
};
use super::super::instruments::{enum_from_str, serialize_typed_instrument_json};
use super::super::typed_fx::{
    instrument_envelope_methods, instrument_pricing_methods, take_builder,
};
use super::super::typed_legs::{PyPremiumLegSpec, PyProtectionLegSpec};

type CdsBuilderInner =
    finstack_quant_valuations::instruments::credit_derivatives::cds::CreditDefaultSwapBuilder;

/// Parse a CDS regional convention, listing the accepted strings on failure.
///
/// `"isda_na"` is the SNAC / post-Big-Bang North American standard and the
/// Rust default; analysts who type `"snac"` or `"isda_2014"` get the list.
pub(crate) fn cds_convention_from_str(value: &str) -> PyResult<CdsConvention> {
    enum_from_str::<CdsConvention>(value, "convention").map_err(|_| {
        value_error(format!(
            "invalid convention {value:?}: expected one of 'isda_na' (SNAC / post-Big-Bang \
             North American standard: ACT/360, quarterly IMM, T+3), 'isda_eu' (European: \
             T+1, TARGET2), 'isda_as' (Asian: ACT/365F, Tokyo) or 'custom'"
        ))
    })
}

/// Single-name credit default swap (typed wrapper for Rust ``CreditDefaultSwap``).
///
/// Follows the ISDA CDS Standard Model conventions: quarterly IMM premium
/// dates, ACT/360, accrual-on-default, points-upfront quoting via
/// ``upfront``. ``convention="isda_na"`` is the SNAC / post-Big-Bang
/// standard and the default; ``valuation_convention`` defaults to Bloomberg
/// CDSW clean principal.
///
/// Build with ``CreditDefaultSwap.builder()`` or start from
/// ``CreditDefaultSwap.example()``; instances are accepted directly by
/// ``price_instrument`` and expose ``price`` / ``metric`` themselves. The
/// ``"cs01"`` and ``"bucketed_cs01"`` metrics rebootstrap a quote-backed
/// hazard curve from its stored calibration recipe.
///
/// Examples
/// --------
/// >>> from finstack_quant.valuations.instruments import CreditDefaultSwap
/// >>> cds = CreditDefaultSwap.example()
/// >>> (cds.id, cds.side, cds.convention)
/// ('CDS-CORP-5Y', 'pay', 'isda_na')
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "CreditDefaultSwap",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyCreditDefaultSwap {
    /// Inner canonical Rust CDS.
    pub(crate) inner: finstack_quant_valuations::instruments::CreditDefaultSwap,
}

impl PyCreditDefaultSwap {
    /// Serialize as the canonical instrument envelope accepted by the JSON loader.
    pub(crate) fn envelope_json(&self) -> PyResult<String> {
        serialize_typed_instrument_json(
            InstrumentJson::CreditDefaultSwap(self.inner.clone()),
            "CreditDefaultSwap",
        )
    }
}

instrument_envelope_methods!(
    PyCreditDefaultSwap,
    CreditDefaultSwap,
    "credit_default_swap",
    PyCreditDefaultSwapBuilder,
    finstack_quant_valuations::instruments::CreditDefaultSwap::builder()
);
instrument_pricing_methods!(PyCreditDefaultSwap);

#[pymethods]
impl PyCreditDefaultSwap {
    /// Canonical example: 5-year USD 10,000,000 investment-grade payer CDS.
    ///
    /// Mirrors Rust ``CreditDefaultSwap::example()``: ``isda_na`` convention,
    /// 100bp running spread, 40% recovery, curves ``USD-OIS`` /
    /// ``CORP-HAZARD``, premium 2024-03-20 to 2029-03-20.
    ///
    /// Returns
    /// -------
    /// CreditDefaultSwap
    ///     The validated example CDS.
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn example() -> Self {
        Self {
            inner: finstack_quant_valuations::instruments::CreditDefaultSwap::example(),
        }
    }

    /// Par spread implied by the market, in basis points.
    ///
    /// Mirrors Rust ``CreditDefaultSwap::get_par_spread``: the running
    /// spread that makes the contract worth zero under this CDS's valuation
    /// convention, premium schedule, discount curve, hazard curve and
    /// recovery assumption.
    ///
    /// Parameters
    /// ----------
    /// market : MarketContext | str
    ///     Market carrying the discount and hazard curves named by the CDS.
    /// as_of : datetime.date | str
    ///     Valuation date.
    ///
    /// Returns
    /// -------
    /// float
    ///     Par spread in basis points.
    ///
    /// Raises
    /// ------
    /// KeyError
    ///     If a curve is missing from ``market``.
    /// ValueError
    ///     If the curve recovery metadata conflicts with the contract recovery.
    #[pyo3(text_signature = "($self, market, as_of)")]
    fn get_par_spread(
        &self,
        py: Python<'_>,
        market: &Bound<'_, PyAny>,
        as_of: &Bound<'_, PyAny>,
    ) -> PyResult<f64> {
        let market = extract_market(py, market)?;
        let as_of = extract_date(as_of)?;
        self.inner
            .get_par_spread(&market, as_of)
            .map_err(core_to_py)
    }

    /// Notional amount of protection.
    #[getter]
    fn notional(&self) -> PyMoney {
        money_to_py(self.inner.notional)
    }

    /// ``"pay"`` (buy protection) or ``"receive"`` (sell protection).
    #[getter]
    fn side(&self) -> PyResult<String> {
        enum_to_py_string(&self.inner.side)
    }

    /// ISDA regional convention (``"isda_na"``, ``"isda_eu"``, ``"isda_as"``, ``"custom"``).
    #[getter]
    fn convention(&self) -> PyResult<String> {
        enum_to_py_string(&self.inner.convention)
    }

    /// Premium leg specification.
    #[getter]
    fn premium(&self) -> PyPremiumLegSpec {
        PyPremiumLegSpec {
            inner: self.inner.premium.clone(),
        }
    }

    /// Protection leg specification.
    #[getter]
    fn protection(&self) -> PyProtectionLegSpec {
        PyProtectionLegSpec {
            inner: self.inner.protection.clone(),
        }
    }

    /// Valuation presentation convention (serde name, default ``"bloomberg_cdsw_clean"``).
    #[getter]
    fn valuation_convention(&self) -> PyResult<String> {
        enum_to_py_string(&self.inner.valuation_convention)
    }

    /// Points-upfront payment as ``(payment_date, amount)``, or ``None``.
    ///
    /// Positive means the protection buyer pays the seller.
    #[getter]
    fn upfront<'py>(&self, py: Python<'py>) -> PyResult<Option<(Bound<'py, PyAny>, PyMoney)>> {
        self.inner
            .upfront
            .map(|(date, money)| Ok((date_to_py(py, date)?, money_to_py(money))))
            .transpose()
    }

    /// Explicit ISDA documentation clause, or ``None`` when derived from the convention.
    #[getter]
    fn doc_clause(&self) -> PyResult<Option<String>> {
        self.inner
            .doc_clause
            .map(|clause| enum_to_py_string(&clause))
            .transpose()
    }

    /// Effective documentation clause after convention-based resolution.
    ///
    /// Mirrors Rust ``CreditDefaultSwap::doc_clause_effective``: ``"xr14"``
    /// for ``isda_na`` / ``isda_as`` / ``custom``, ``"mm14"`` for ``isda_eu``.
    #[getter]
    fn doc_clause_effective(&self) -> PyResult<String> {
        enum_to_py_string(&self.inner.doc_clause_effective())
    }

    /// Protection effective date for a forward-starting CDS, or ``None``.
    #[getter]
    fn protection_effective_date<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .protection_effective_date
            .map(|d| date_to_py(py, d))
            .transpose()
    }

    /// Date protection starts (``protection_effective_date`` or the premium start).
    #[getter]
    fn protection_start<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.protection_start())
    }

    /// OTC margin specification as a dict, or ``None`` for unmargined trades.
    #[getter]
    fn margin_spec<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .margin_spec
            .as_ref()
            .map(|spec| crate::bindings::pandas_utils::serde_to_py(py, spec))
            .transpose()
    }

    /// Premium-leg end date (the pricer's notion of expiry), or ``None``.
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        Instrument::expiry(&self.inner)
            .map(|d| date_to_py(py, d))
            .transpose()
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "CreditDefaultSwap(id={:?}, side={:?}, notional={}, spread_bp={}, start={}, end={}, convention={:?})",
            self.inner.id.as_str(),
            enum_to_py_string(&self.inner.side).unwrap_or_default(),
            money_repr(self.inner.notional),
            opt_repr(self.inner.premium.spread_bp.to_f64()),
            date_repr(self.inner.premium.start),
            date_repr(self.inner.premium.end),
            enum_to_py_string(&self.inner.convention).unwrap_or_default(),
        )
    }
}

/// Fluent builder for ``CreditDefaultSwap``; wraps the Rust
/// ``FinancialBuilder``-generated builder (consuming setters).
///
/// Builders are consumed by ``build()``; create a new builder per instrument.
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "CreditDefaultSwapBuilder",
    skip_from_py_object
)]
pub struct PyCreditDefaultSwapBuilder {
    inner: Option<CdsBuilderInner>,
    fields: Vec<(&'static str, String)>,
}

/// Apply one consuming Rust setter and record the field for ``__repr__``.
macro_rules! cds_set {
    ($slf:ident, $field:ident, $repr:expr, $apply:expr) => {{
        let b = take_builder(&mut $slf.inner)?;
        $slf.inner = Some($apply(b));
        $slf.fields.push((stringify!($field), $repr));
        Ok($slf)
    }};
}

#[pymethods]
impl PyCreditDefaultSwapBuilder {
    /// Set the instrument identifier.
    ///
    /// Parameters
    /// ----------
    /// value : str
    ///     Unique identifier for the CDS.
    ///
    /// Returns
    /// -------
    /// CreditDefaultSwapBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        cds_set!(slf, id, format!("{value:?}"), |b: CdsBuilderInner| b
            .id(InstrumentId::new(value.to_string())))
    }

    /// Set the notional amount.
    ///
    /// Parameters
    /// ----------
    /// value : Money
    ///     Notional amount of protection.
    ///
    /// Returns
    /// -------
    /// CreditDefaultSwapBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn notional<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: PyRef<'_, PyMoney>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let money = value.inner;
        cds_set!(slf, notional, money_repr(money), |b: CdsBuilderInner| b
            .notional(money))
    }

    /// Set the protection buyer/seller perspective.
    ///
    /// Parameters
    /// ----------
    /// value : {"pay", "receive"}
    ///     ``"pay"`` to buy protection (pay premium), ``"receive"`` to sell
    ///     protection (receive premium).
    ///
    /// Returns
    /// -------
    /// CreditDefaultSwapBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` is not a recognized side.
    #[pyo3(text_signature = "($self, value)")]
    fn side<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let side = enum_from_str(value, "side")?;
        cds_set!(slf, side, format!("{value:?}"), |b: CdsBuilderInner| b
            .side(side))
    }

    /// Set the ISDA regional convention.
    ///
    /// Parameters
    /// ----------
    /// value : {"isda_na", "isda_eu", "isda_as", "custom"}
    ///     ``"isda_na"`` is the SNAC / post-Big-Bang North American standard
    ///     (ACT/360, quarterly IMM, T+3); ``"isda_eu"`` the European standard
    ///     (T+1, TARGET2); ``"isda_as"`` Asian (ACT/365F, Tokyo); ``"custom"``
    ///     for a manually configured convention.
    ///
    /// Returns
    /// -------
    /// CreditDefaultSwapBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` is not one of the accepted strings; the message lists them.
    #[pyo3(text_signature = "($self, value)")]
    fn convention<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let convention = cds_convention_from_str(value)?;
        cds_set!(
            slf,
            convention,
            format!("{value:?}"),
            |b: CdsBuilderInner| b.convention(convention)
        )
    }

    /// Set the premium leg specification.
    ///
    /// Parameters
    /// ----------
    /// value : PremiumLegSpec
    ///     Premium leg specification.
    ///
    /// Returns
    /// -------
    /// CreditDefaultSwapBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn premium<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: PyRef<'_, PyPremiumLegSpec>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let leg = value.inner.clone();
        let shown = format!(
            "PremiumLegSpec(spread_bp={}, start={}, end={})",
            leg.spread_bp,
            date_repr(leg.start),
            date_repr(leg.end)
        );
        cds_set!(slf, premium, shown, |b: CdsBuilderInner| b.premium(leg))
    }

    /// Set the protection leg specification.
    ///
    /// Parameters
    /// ----------
    /// value : ProtectionLegSpec
    ///     Protection leg specification.
    ///
    /// Returns
    /// -------
    /// CreditDefaultSwapBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn protection<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: PyRef<'_, PyProtectionLegSpec>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let leg = value.inner.clone();
        let shown = format!(
            "ProtectionLegSpec(credit_curve_id={:?}, recovery_rate={})",
            leg.credit_curve_id.as_str(),
            leg.recovery_rate
        );
        cds_set!(slf, protection, shown, |b: CdsBuilderInner| b
            .protection(leg))
    }

    /// Set the valuation presentation convention.
    ///
    /// Parameters
    /// ----------
    /// value : {"bloomberg_cdsw_clean", "bloomberg_cdsw_clean_full_premium", "isda_dirty", "quant_lib_isda_parity"}
    ///     ``"bloomberg_cdsw_clean"`` (default) reports Bloomberg CDSW clean
    ///     principal; ``"isda_dirty"`` the academic ISDA dirty PV;
    ///     ``"quant_lib_isda_parity"`` reproduces QuantLib ``IsdaCdsEngine``.
    ///
    /// Returns
    /// -------
    /// CreditDefaultSwapBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` is not a recognized valuation convention.
    #[pyo3(text_signature = "($self, value)")]
    fn valuation_convention<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let convention: CdsValuationConvention = enum_from_str(value, "valuation_convention")?;
        cds_set!(
            slf,
            valuation_convention,
            format!("{value:?}"),
            |b: CdsBuilderInner| b.valuation_convention(convention)
        )
    }

    /// Set the points-upfront payment (the standard post-Big-Bang quote).
    ///
    /// Parameters
    /// ----------
    /// value : tuple[datetime.date | str, Money]
    ///     ``(payment_date, amount)``. The amount is a payment from protection
    ///     buyer to protection seller: positive means the buyer pays, negative
    ///     means the seller pays. Its currency must match the notional.
    ///
    /// Returns
    /// -------
    /// CreditDefaultSwapBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// TypeError
    ///     If ``value`` is not a ``(date, Money)`` pair.
    #[pyo3(text_signature = "($self, value)")]
    fn upfront<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let (date, money) = dated_money_from_py(value, None, "upfront")?;
        let shown = format!("({}, {})", date_repr(date), money_repr(money));
        cds_set!(slf, upfront, shown, |b: CdsBuilderInner| b
            .upfront((date, money)))
    }

    /// Set the ISDA documentation clause for restructuring credit events.
    ///
    /// Parameters
    /// ----------
    /// value : {"cr14", "mr14", "mm14", "xr14", "isda_na", "isda_eu", "isda_as", "isda_au", "isda_nz", "custom"}
    ///     Restructuring documentation clause: one of the four 2014 ISDA
    ///     restructuring elections (``"cr14"``/``"mr14"``/``"mm14"``/
    ///     ``"xr14"``), a regional ISDA corporate default (``"isda_na"``/
    ///     ``"isda_eu"``/``"isda_as"``/``"isda_au"``/``"isda_nz"``), or
    ///     ``"custom"``. If never set, the effective clause is derived from
    ///     the CDS convention (see ``doc_clause_effective``).
    ///
    /// Returns
    /// -------
    /// CreditDefaultSwapBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` is not a recognized documentation clause.
    #[pyo3(text_signature = "($self, value)")]
    fn doc_clause<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let clause: CdsDocClause = enum_from_str(value, "doc_clause")?;
        cds_set!(
            slf,
            doc_clause,
            format!("{value:?}"),
            |b: CdsBuilderInner| b.doc_clause(clause)
        )
    }

    /// Set the protection effective date for a forward-starting CDS.
    ///
    /// Parameters
    /// ----------
    /// value : datetime.date | str
    ///     Date on which credit protection begins. Must satisfy
    ///     ``premium.start <= value <= premium.end``. When never set,
    ///     protection starts on the premium leg start date.
    ///
    /// Returns
    /// -------
    /// CreditDefaultSwapBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn protection_effective_date<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let date = extract_date(value)?;
        cds_set!(
            slf,
            protection_effective_date,
            date_repr(date),
            |b: CdsBuilderInner| b.protection_effective_date(date)
        )
    }

    /// Set the OTC margin specification for VM/IM.
    ///
    /// Parameters
    /// ----------
    /// value : dict | str
    ///     ``OtcMarginSpec`` as a dict or JSON string. Cleared CDS use the
    ///     ``cleared`` form (CCP + currency); bilateral CDS need a SIMM
    ///     credit classification so CS01 is routed to the right bucket.
    ///
    /// Returns
    /// -------
    /// CreditDefaultSwapBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` does not deserialize as an ``OtcMarginSpec``.
    #[pyo3(text_signature = "($self, value)")]
    fn margin_spec<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'py>,
        value: &Bound<'py, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let spec = crate::bindings::module_utils::py_to_serde(py, value, "margin_spec")?;
        let shown = value.repr()?.to_string();
        cds_set!(slf, margin_spec, shown, |b: CdsBuilderInner| b
            .margin_spec(spec))
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
    /// CreditDefaultSwapBuilder
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
        cds_set!(slf, attributes, shown, |b: CdsBuilderInner| b
            .attributes(attrs))
    }

    /// Build the validated CDS.
    ///
    /// Validation is the Rust ``CreditDefaultSwap::builder().build()``
    /// invariants only; there is no additional binding-side check.
    ///
    /// Returns
    /// -------
    /// CreditDefaultSwap
    ///     The validated CDS.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the builder was already consumed, a required field is missing,
    ///     or the completed CDS fails validation (recovery outside ``[0, 1]``,
    ///     upfront currency mismatch, protection date outside the premium period).
    #[pyo3(text_signature = "($self)")]
    fn build(mut slf: PyRefMut<'_, Self>) -> PyResult<PyCreditDefaultSwap> {
        let b = take_builder(&mut slf.inner)?;
        let inner = b.build().map_err(core_to_py)?;
        Ok(PyCreditDefaultSwap { inner })
    }

    /// Return ``repr(self)`` listing the fields set so far.
    fn __repr__(&self) -> String {
        builder_repr("CreditDefaultSwapBuilder", &self.fields)
    }
}
