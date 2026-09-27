//! CDS tranche Python wrappers: `CDSTranche`, its fluent builder and the
//! `CDSTrancheParams` descriptor used by `CDSTranche.standard`.

use pyo3::prelude::*;

use crate::bindings::cashflows::builder::specs::PyRollRule;
use crate::bindings::core::dates::schedule::PyStubKind;
use crate::bindings::core::dates::tenor::PyTenor;
use crate::bindings::core::money::PyMoney;
use crate::bindings::date_utils::{date_to_py, extract_date};
use crate::bindings::extract::extract_market;
use crate::errors::core_to_py;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_valuations::instruments::credit_derivatives::cds_tranche::CDSTrancheParams;
use finstack_quant_valuations::instruments::PayReceive;
use finstack_quant_valuations::instruments::{Instrument, InstrumentJson};

use super::super::convert::{
    attributes_from_py, bdc_from_py, bps_from_py, builder_repr, date_repr, dated_money_from_py,
    day_count_from_py, enum_to_py_string, float_repr, money_from_py, money_repr, money_to_py,
    tenor_from_py,
};
use super::super::instruments::{
    enum_from_str, serialize_typed_instrument_json, stub_kind_from_py,
};
use super::super::typed_fx::{
    instrument_envelope_methods, instrument_pricing_methods, take_builder,
};

type CdsTrancheBuilderInner =
    finstack_quant_valuations::instruments::credit_derivatives::cds_tranche::CDSTrancheBuilder;

/// Economic terms of an index tranche (typed wrapper for Rust ``CDSTrancheParams``).
///
/// Attachment and detachment are quoted in percent points (``3.0`` = 3%),
/// the running coupon in basis points. Pass to ``CDSTranche.standard`` for a
/// tranche on the standard quarterly ACT/360 schedule.
///
/// Examples
/// --------
/// >>> import datetime
/// >>> from finstack_quant.core.currency import Currency
/// >>> from finstack_quant.core.money import Money
/// >>> from finstack_quant.valuations.instruments import CDSTrancheParams
/// >>> params = CDSTrancheParams.mezzanine_tranche(
/// ...     "CDX.NA.IG", 42, Money(10_000_000.0, Currency("USD")), datetime.date(2029, 12, 20), 100.0
/// ... )
/// >>> (params.attach_pct, params.detach_pct)
/// (3.0, 7.0)
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "CDSTrancheParams",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyCDSTrancheParams {
    /// Inner canonical Rust tranche parameters.
    pub(crate) inner: CDSTrancheParams,
}

#[pymethods]
impl PyCDSTrancheParams {
    /// Describe a tranche on a credit index.
    ///
    /// Parameters
    /// ----------
    /// index_name : str
    ///     Underlying index name, e.g. ``"CDX.NA.IG"``.
    /// series : int
    ///     Index series number.
    /// attach_pct : float
    ///     Attachment point in percent (``3.0`` = 3%).
    /// detach_pct : float
    ///     Detachment point in percent (``7.0`` = 7%); must exceed ``attach_pct``.
    /// notional : Money
    ///     Tranche notional.
    /// maturity : datetime.date | str
    ///     Scheduled maturity (an IMM date for standard tranches).
    /// coupon_bp : float | Bps
    ///     Running coupon in basis points (``100.0`` = 1%).
    /// realized_loss : float
    ///     Realized portfolio loss so far as a fraction of the original
    ///     portfolio notional, in ``[0, 1]``; default ``0.0``.
    ///
    /// Returns
    /// -------
    /// CDSTrancheParams
    ///     The tranche terms.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``realized_loss`` is outside ``[0, 1]``.
    /// TypeError
    ///     If ``coupon_bp`` is neither a number nor ``Bps``.
    ///
    /// Examples
    /// --------
    /// >>> import datetime
    /// >>> from finstack_quant.core.currency import Currency
    /// >>> from finstack_quant.core.money import Money
    /// >>> from finstack_quant.valuations.instruments import CDSTrancheParams
    /// >>> params = CDSTrancheParams(
    /// ...     "CDX.NA.IG", 42, 7.0, 15.0, Money(5_000_000.0, Currency("USD")), "2029-12-20", 100.0
    /// ... )
    /// >>> params.coupon_bp
    /// 100.0
    #[new]
    #[pyo3(signature = (index_name, series, attach_pct, detach_pct, notional, maturity, coupon_bp, realized_loss=0.0))]
    #[pyo3(
        text_signature = "(index_name, series, attach_pct, detach_pct, notional, maturity, coupon_bp, realized_loss=0.0)"
    )]
    // PyO3 binding: the argument list mirrors the Rust constructor one-for-one.
    #[allow(clippy::too_many_arguments)]
    fn new(
        index_name: &str,
        series: u16,
        attach_pct: f64,
        detach_pct: f64,
        notional: &Bound<'_, PyAny>,
        maturity: &Bound<'_, PyAny>,
        coupon_bp: &Bound<'_, PyAny>,
        realized_loss: f64,
    ) -> PyResult<Self> {
        let inner = CDSTrancheParams::new(
            index_name,
            series,
            attach_pct,
            detach_pct,
            money_from_py(notional, None, "notional")?,
            extract_date(maturity)?,
            bps_from_py(coupon_bp, "coupon_bp")?,
        )
        .with_realized_loss(realized_loss)
        .map_err(core_to_py)?;
        Ok(Self { inner })
    }

    /// Standard equity tranche (0%–3%).
    ///
    /// Parameters
    /// ----------
    /// index_name : str
    ///     Underlying index name.
    /// series : int
    ///     Index series number.
    /// notional : Money
    ///     Tranche notional.
    /// maturity : datetime.date | str
    ///     Scheduled maturity.
    /// coupon_bp : float | Bps
    ///     Running coupon in basis points.
    ///
    /// Returns
    /// -------
    /// CDSTrancheParams
    ///     Tranche terms with ``attach_pct=0.0`` and ``detach_pct=3.0``.
    ///
    /// Raises
    /// ------
    /// TypeError
    ///     If ``coupon_bp`` is neither a number nor ``Bps``.
    ///
    /// Examples
    /// --------
    /// >>> from finstack_quant.core.currency import Currency
    /// >>> from finstack_quant.core.money import Money
    /// >>> from finstack_quant.valuations.instruments import CDSTrancheParams
    /// >>> CDSTrancheParams.equity_tranche(
    /// ...     "CDX.NA.IG", 42, Money(1e7, Currency("USD")), "2029-12-20", 500.0
    /// ... ).detach_pct
    /// 3.0
    #[staticmethod]
    #[pyo3(text_signature = "(index_name, series, notional, maturity, coupon_bp)")]
    fn equity_tranche(
        index_name: &str,
        series: u16,
        notional: &Bound<'_, PyAny>,
        maturity: &Bound<'_, PyAny>,
        coupon_bp: &Bound<'_, PyAny>,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: CDSTrancheParams::equity_tranche(
                index_name,
                series,
                money_from_py(notional, None, "notional")?,
                extract_date(maturity)?,
                bps_from_py(coupon_bp, "coupon_bp")?,
            ),
        })
    }

    /// Standard mezzanine tranche (3%–7%).
    ///
    /// Parameters
    /// ----------
    /// index_name : str
    ///     Underlying index name.
    /// series : int
    ///     Index series number.
    /// notional : Money
    ///     Tranche notional.
    /// maturity : datetime.date | str
    ///     Scheduled maturity.
    /// coupon_bp : float | Bps
    ///     Running coupon in basis points.
    ///
    /// Returns
    /// -------
    /// CDSTrancheParams
    ///     Tranche terms with ``attach_pct=3.0`` and ``detach_pct=7.0``.
    ///
    /// Raises
    /// ------
    /// TypeError
    ///     If ``coupon_bp`` is neither a number nor ``Bps``.
    ///
    /// Examples
    /// --------
    /// >>> from finstack_quant.core.currency import Currency
    /// >>> from finstack_quant.core.money import Money
    /// >>> from finstack_quant.valuations.instruments import CDSTrancheParams
    /// >>> CDSTrancheParams.mezzanine_tranche(
    /// ...     "CDX.NA.IG", 42, Money(1e7, Currency("USD")), "2029-12-20", 100.0
    /// ... ).attach_pct
    /// 3.0
    #[staticmethod]
    #[pyo3(text_signature = "(index_name, series, notional, maturity, coupon_bp)")]
    fn mezzanine_tranche(
        index_name: &str,
        series: u16,
        notional: &Bound<'_, PyAny>,
        maturity: &Bound<'_, PyAny>,
        coupon_bp: &Bound<'_, PyAny>,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: CDSTrancheParams::mezzanine_tranche(
                index_name,
                series,
                money_from_py(notional, None, "notional")?,
                extract_date(maturity)?,
                bps_from_py(coupon_bp, "coupon_bp")?,
            ),
        })
    }

    /// Underlying index name.
    #[getter]
    fn index_name(&self) -> String {
        self.inner.index_name.clone()
    }

    /// Index series number.
    #[getter]
    fn series(&self) -> u16 {
        self.inner.series
    }

    /// Attachment point in percent.
    #[getter]
    fn attach_pct(&self) -> f64 {
        self.inner.attach_pct
    }

    /// Detachment point in percent.
    #[getter]
    fn detach_pct(&self) -> f64 {
        self.inner.detach_pct
    }

    /// Tranche notional.
    #[getter]
    fn notional(&self) -> PyMoney {
        money_to_py(self.inner.notional)
    }

    /// Scheduled maturity.
    #[getter]
    fn maturity<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.maturity)
    }

    /// Running coupon in basis points.
    #[getter]
    fn coupon_bp(&self) -> f64 {
        self.inner.coupon_bp
    }

    /// Realized portfolio loss so far (fraction of original notional).
    #[getter]
    fn realized_loss(&self) -> f64 {
        self.inner.realized_loss
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "CDSTrancheParams(index_name={:?}, series={}, attach_pct={}, detach_pct={}, notional={}, maturity={}, coupon_bp={}, realized_loss={})",
            self.inner.index_name,
            self.inner.series,
            float_repr(self.inner.attach_pct),
            float_repr(self.inner.detach_pct),
            money_repr(self.inner.notional),
            date_repr(self.inner.maturity),
            float_repr(self.inner.coupon_bp),
            float_repr(self.inner.realized_loss),
        )
    }
}

/// Synthetic CDO / index tranche (typed wrapper for Rust ``CDSTranche``).
///
/// Protection on portfolio losses between ``attach_pct`` and ``detach_pct``
/// (percent points), paying ``coupon_bp`` on the surviving tranche
/// notional. Priced with the one-factor Gaussian copula against the
/// ``credit_index_id`` loss distribution.
///
/// Build with ``CDSTranche.builder()`` or ``CDSTranche.standard(...)``; start
/// from ``CDSTranche.example()``. Instances are accepted directly by
/// ``price_instrument`` and expose ``price`` / ``metric`` /
/// ``expected_loss`` / ``jump_to_default`` themselves.
///
/// Examples
/// --------
/// >>> from finstack_quant.valuations.instruments import CDSTranche
/// >>> tranche = CDSTranche.example()
/// >>> (tranche.attach_pct, tranche.detach_pct, tranche.side)
/// (0.0, 3.0, 'pay')
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "CDSTranche",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyCDSTranche {
    /// Inner canonical Rust CDS tranche.
    pub(crate) inner: finstack_quant_valuations::instruments::CDSTranche,
}

impl PyCDSTranche {
    /// Serialize as the canonical instrument envelope accepted by the JSON loader.
    pub(crate) fn envelope_json(&self) -> PyResult<String> {
        serialize_typed_instrument_json(
            InstrumentJson::CDSTranche(self.inner.clone()),
            "CDSTranche",
        )
    }
}

instrument_envelope_methods!(
    PyCDSTranche,
    CDSTranche,
    "cds_tranche",
    PyCDSTrancheBuilder,
    finstack_quant_valuations::instruments::CDSTranche::builder().realized_loss(0.0)
);
instrument_pricing_methods!(PyCDSTranche);

#[pymethods]
impl PyCDSTranche {
    /// Canonical example: CDX.NA.IG 42 equity (0–3%) tranche, USD 10,000,000.
    ///
    /// Mirrors Rust ``CDSTranche::example()``: buy protection, 100bp running,
    /// maturity 2029-12-20, curves ``USD-OIS`` / ``CDX.NA.IG.HAZARD``.
    ///
    /// Returns
    /// -------
    /// CDSTranche
    ///     The example tranche.
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn example() -> Self {
        Self {
            inner: finstack_quant_valuations::instruments::CDSTranche::example(),
        }
    }

    /// Build a tranche on the standard schedule.
    ///
    /// Mirrors Rust ``CDSTranche::standard``: quarterly, ACT/360, Following,
    /// weekends-only calendar, short-front stub.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Unique instrument identifier.
    /// params : CDSTrancheParams
    ///     Economic terms (attach/detach, notional, maturity, coupon).
    /// discount_curve_id : str
    ///     Discount curve identifier.
    /// credit_index_id : str
    ///     Credit index identifier for the loss distribution.
    /// side : {"pay", "receive"}
    ///     Tranche side: ``"pay"`` buys protection, ``"receive"`` sells it.
    ///
    /// Returns
    /// -------
    /// CDSTranche
    ///     The validated tranche.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``side`` is unknown or the parameters fail validation
    ///     (``attach_pct >= detach_pct``, fractional attach/detach, …).
    #[staticmethod]
    #[pyo3(text_signature = "(id, params, discount_curve_id, credit_index_id, side)")]
    fn standard(
        id: &str,
        params: PyRef<'_, PyCDSTrancheParams>,
        discount_curve_id: &str,
        credit_index_id: &str,
        side: &str,
    ) -> PyResult<Self> {
        let side: PayReceive = enum_from_str(side, "side")?;
        let inner = finstack_quant_valuations::instruments::CDSTranche::standard(
            InstrumentId::new(id.to_string()),
            &params.inner,
            CurveId::new(discount_curve_id.to_string()),
            CurveId::new(credit_index_id.to_string()),
            side,
        )
        .map_err(core_to_py)?;
        Ok(Self { inner })
    }

    /// Expected tranche loss as a fraction of tranche notional.
    ///
    /// Mirrors Rust ``CDSTranche::expected_loss``.
    ///
    /// Parameters
    /// ----------
    /// market : MarketContext | str
    ///     Market carrying the credit index and discount curve.
    ///
    /// Returns
    /// -------
    /// float
    ///     Expected loss fraction in ``[0, 1]``.
    ///
    /// Raises
    /// ------
    /// KeyError
    ///     If the credit index is missing from ``market``.
    /// RuntimeError
    ///     If the loss-distribution integration fails.
    #[pyo3(text_signature = "($self, market)")]
    fn expected_loss(&self, py: Python<'_>, market: &Bound<'_, PyAny>) -> PyResult<f64> {
        let market = extract_market(py, market)?;
        self.inner.expected_loss(&market).map_err(core_to_py)
    }

    /// Jump-to-default exposure of the tranche.
    ///
    /// Mirrors Rust ``CDSTranche::jump_to_default``: PV impact of one
    /// constituent defaulting immediately.
    ///
    /// Parameters
    /// ----------
    /// market : MarketContext | str
    ///     Market carrying the credit index and discount curve.
    /// as_of : datetime.date | str
    ///     Valuation date.
    ///
    /// Returns
    /// -------
    /// float
    ///     Jump-to-default PV change in notional currency units.
    ///
    /// Raises
    /// ------
    /// KeyError
    ///     If the credit index is missing from ``market``.
    /// RuntimeError
    ///     If the loss-distribution integration fails.
    #[pyo3(text_signature = "($self, market, as_of)")]
    fn jump_to_default(
        &self,
        py: Python<'_>,
        market: &Bound<'_, PyAny>,
        as_of: &Bound<'_, PyAny>,
    ) -> PyResult<f64> {
        let market = extract_market(py, market)?;
        let as_of = extract_date(as_of)?;
        self.inner
            .jump_to_default(&market, as_of)
            .map_err(core_to_py)
    }

    /// Underlying index name.
    #[getter]
    fn index_name(&self) -> String {
        self.inner.index_name.clone()
    }

    /// Index series number.
    #[getter]
    fn series(&self) -> u16 {
        self.inner.series
    }

    /// Attachment point in percent.
    #[getter]
    fn attach_pct(&self) -> f64 {
        self.inner.attach_pct
    }

    /// Detachment point in percent.
    #[getter]
    fn detach_pct(&self) -> f64 {
        self.inner.detach_pct
    }

    /// Tranche notional.
    #[getter]
    fn notional(&self) -> PyMoney {
        money_to_py(self.inner.notional)
    }

    /// Scheduled maturity.
    #[getter]
    fn maturity<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.maturity)
    }

    /// Running coupon in basis points.
    #[getter]
    fn coupon_bp(&self) -> f64 {
        self.inner.coupon_bp
    }

    /// Payment frequency.
    #[getter]
    fn frequency(&self) -> PyTenor {
        PyTenor::from_inner(self.inner.frequency)
    }

    /// Day count convention (serde name, e.g. ``"act_360"``).
    #[getter]
    fn day_count(&self) -> PyResult<String> {
        enum_to_py_string(&self.inner.day_count)
    }

    /// Business day convention (serde name, default ``"modified_following"``).
    #[getter]
    fn business_day_convention(&self) -> PyResult<String> {
        enum_to_py_string(&self.inner.business_day_convention)
    }

    /// Holiday calendar identifier, if any.
    #[getter]
    fn calendar_id(&self) -> Option<String> {
        self.inner.calendar_id.as_ref().map(ToString::to_string)
    }

    /// Discount curve identifier.
    #[getter]
    fn discount_curve_id(&self) -> String {
        self.inner.discount_curve_id.to_string()
    }

    /// Credit index identifier for the loss distribution.
    #[getter]
    fn credit_index_id(&self) -> String {
        self.inner.credit_index_id.to_string()
    }

    /// ``"pay"`` (buy protection) or ``"receive"`` (sell protection).
    #[getter]
    fn side(&self) -> PyResult<String> {
        enum_to_py_string(&self.inner.side)
    }

    /// Explicit contract start (effective) date for schedule anchoring, or ``None``.
    #[getter]
    fn start_date<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner.start_date.map(|d| date_to_py(py, d)).transpose()
    }

    /// Realized portfolio loss so far (fraction of original notional).
    #[getter]
    fn realized_loss(&self) -> f64 {
        self.inner.realized_loss
    }

    /// Coupon roll-date grid: ``RollRule.CDS_IMM`` (CDS roll dates, 20th of
    /// Mar/Jun/Sep/Dec) or ``RollRule.NONE`` (bespoke ``frequency``/``stub``).
    #[getter]
    fn roll_rule(&self) -> PyRollRule {
        PyRollRule {
            inner: self.inner.roll_rule,
        }
    }

    /// Stub convention for a bespoke (``RollRule.NONE``) coupon schedule.
    #[getter]
    fn stub(&self) -> PyStubKind {
        PyStubKind::from_inner(self.inner.stub)
    }

    /// Upfront payment as ``(payment_date, amount)``, or ``None``.
    #[getter]
    fn upfront<'py>(&self, py: Python<'py>) -> PyResult<Option<(Bound<'py, PyAny>, PyMoney)>> {
        self.inner
            .upfront
            .map(|(date, money)| Ok((date_to_py(py, date)?, money_to_py(money))))
            .transpose()
    }

    /// Maturity as seen by the pricer, or ``None``.
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        Instrument::expiry(&self.inner)
            .map(|d| date_to_py(py, d))
            .transpose()
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "CDSTranche(id={:?}, index_name={:?}, series={}, attach_pct={}, detach_pct={}, side={:?}, notional={}, coupon_bp={}, maturity={})",
            self.inner.id.as_str(),
            self.inner.index_name,
            self.inner.series,
            float_repr(self.inner.attach_pct),
            float_repr(self.inner.detach_pct),
            enum_to_py_string(&self.inner.side).unwrap_or_default(),
            money_repr(self.inner.notional),
            float_repr(self.inner.coupon_bp),
            date_repr(self.inner.maturity),
        )
    }
}

/// Fluent builder for ``CDSTranche``; wraps the Rust
/// ``FinancialBuilder``-generated builder (consuming setters).
///
/// The builder pre-seeds ``realized_loss(0.0)``; ``roll_rule`` defaults to
/// ``RollRule.NONE``, ``stub`` to ``"short_front"`` and
/// ``business_day_convention`` to
/// ``"modified_following"``. Builders are consumed by ``build()``; create a
/// new builder per instrument.
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "CDSTrancheBuilder",
    skip_from_py_object
)]
pub struct PyCDSTrancheBuilder {
    inner: Option<CdsTrancheBuilderInner>,
    fields: Vec<(&'static str, String)>,
}

/// Apply one consuming Rust setter and record the field for ``__repr__``.
macro_rules! tranche_set {
    ($slf:ident, $field:ident, $repr:expr, $apply:expr) => {{
        let b = take_builder(&mut $slf.inner)?;
        $slf.inner = Some($apply(b));
        $slf.fields.push((stringify!($field), $repr));
        Ok($slf)
    }};
}

#[pymethods]
impl PyCDSTrancheBuilder {
    /// Set the instrument identifier.
    ///
    /// Parameters
    /// ----------
    /// value : str
    ///     Unique identifier for the tranche trade.
    ///
    /// Returns
    /// -------
    /// CDSTrancheBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        tranche_set!(
            slf,
            id,
            format!("{value:?}"),
            |b: CdsTrancheBuilderInner| b.id(InstrumentId::new(value.to_string()))
        )
    }

    /// Set the underlying index name.
    ///
    /// Parameters
    /// ----------
    /// value : str
    ///     Index name, e.g. ``"CDX.NA.IG"``, ``"CDX.NA.HY"``, ``"iTraxx EUR"``.
    ///
    /// Returns
    /// -------
    /// CDSTrancheBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn index_name<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        tranche_set!(
            slf,
            index_name,
            format!("{value:?}"),
            |b: CdsTrancheBuilderInner| b.index_name(value.to_string())
        )
    }

    /// Set the series number.
    ///
    /// Parameters
    /// ----------
    /// value : int
    ///     Series number, e.g. ``42``.
    ///
    /// Returns
    /// -------
    /// CDSTrancheBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn series<'py>(mut slf: PyRefMut<'py, Self>, value: u16) -> PyResult<PyRefMut<'py, Self>> {
        tranche_set!(
            slf,
            series,
            value.to_string(),
            |b: CdsTrancheBuilderInner| b.series(value)
        )
    }

    /// Set the attachment point.
    ///
    /// Parameters
    /// ----------
    /// value : float
    ///     Attachment point quoted in percent (e.g. ``0.0`` for equity;
    ///     ``3.0`` for a tranche attaching at 3%).
    ///
    /// Returns
    /// -------
    /// CDSTrancheBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn attach_pct<'py>(mut slf: PyRefMut<'py, Self>, value: f64) -> PyResult<PyRefMut<'py, Self>> {
        tranche_set!(
            slf,
            attach_pct,
            float_repr(value),
            |b: CdsTrancheBuilderInner| b.attach_pct(value)
        )
    }

    /// Set the detachment point.
    ///
    /// Parameters
    /// ----------
    /// value : float
    ///     Detachment point quoted in percent (e.g. ``3.0`` for a 0-3%
    ///     tranche).
    ///
    /// Returns
    /// -------
    /// CDSTrancheBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn detach_pct<'py>(mut slf: PyRefMut<'py, Self>, value: f64) -> PyResult<PyRefMut<'py, Self>> {
        tranche_set!(
            slf,
            detach_pct,
            float_repr(value),
            |b: CdsTrancheBuilderInner| b.detach_pct(value)
        )
    }

    /// Set the notional amount of the tranche.
    ///
    /// Parameters
    /// ----------
    /// value : Money
    ///     Notional amount of the tranche.
    ///
    /// Returns
    /// -------
    /// CDSTrancheBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn notional<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: PyRef<'_, PyMoney>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let money = value.inner;
        tranche_set!(
            slf,
            notional,
            money_repr(money),
            |b: CdsTrancheBuilderInner| b.notional(money)
        )
    }

    /// Set the maturity date of the tranche.
    ///
    /// Parameters
    /// ----------
    /// value : datetime.date | str
    ///     Maturity date (date-like or ISO 8601 string).
    ///
    /// Returns
    /// -------
    /// CDSTrancheBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn maturity<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let maturity = extract_date(value)?;
        tranche_set!(
            slf,
            maturity,
            date_repr(maturity),
            |b: CdsTrancheBuilderInner| b.maturity(maturity)
        )
    }

    /// Set the running coupon.
    ///
    /// Parameters
    /// ----------
    /// value : float | Bps
    ///     Running coupon in basis points (e.g. ``100.0`` = 1.00%).
    ///
    /// Returns
    /// -------
    /// CDSTrancheBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// TypeError
    ///     If ``value`` is neither a number nor ``Bps``.
    #[pyo3(text_signature = "($self, value)")]
    fn coupon_bp<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let bp = bps_from_py(value, "coupon_bp")?;
        tranche_set!(
            slf,
            coupon_bp,
            float_repr(bp),
            |b: CdsTrancheBuilderInner| b.coupon_bp(bp)
        )
    }

    /// Set the payment frequency.
    ///
    /// Parameters
    /// ----------
    /// value : Tenor | str
    ///     Payment frequency (typically quarterly, ``"3M"``).
    ///
    /// Returns
    /// -------
    /// CDSTrancheBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a tenor string cannot be parsed.
    #[pyo3(text_signature = "($self, value)")]
    fn frequency<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let tenor = tenor_from_py(value, "frequency")?;
        tranche_set!(
            slf,
            frequency,
            format!("Tenor({:?})", tenor.to_string()),
            |b: CdsTrancheBuilderInner| b.frequency(tenor)
        )
    }

    /// Set the day count convention.
    ///
    /// Parameters
    /// ----------
    /// value : DayCount | str
    ///     Day count convention (typically ``DayCount.ACT_360`` / ``"act_360"``).
    ///
    /// Returns
    /// -------
    /// CDSTrancheBuilder
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
        tranche_set!(
            slf,
            day_count,
            format!("DayCount('{day_count}')"),
            |b: CdsTrancheBuilderInner| b.day_count(day_count)
        )
    }

    /// Set the business day convention for coupon dates.
    ///
    /// Parameters
    /// ----------
    /// value : BusinessDayConvention | str
    ///     Roll rule (``"modified_following"`` when never set).
    ///
    /// Returns
    /// -------
    /// CDSTrancheBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a string name is not a recognized convention.
    #[pyo3(text_signature = "($self, value)")]
    fn business_day_convention<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let convention = bdc_from_py(value, "business_day_convention")?;
        let shown = enum_to_py_string(&convention).map(|s| format!("{s:?}"))?;
        tranche_set!(
            slf,
            business_day_convention,
            shown,
            |b: CdsTrancheBuilderInner| b.business_day_convention(convention)
        )
    }

    /// Set the holiday calendar identifier.
    ///
    /// Parameters
    /// ----------
    /// value : str
    ///     Holiday calendar identifier.
    ///
    /// Returns
    /// -------
    /// CDSTrancheBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn calendar_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        tranche_set!(
            slf,
            calendar_id,
            format!("{value:?}"),
            |b: CdsTrancheBuilderInner| b.calendar_id(value.into())
        )
    }

    /// Set the discount curve identifier (by quote currency).
    ///
    /// Parameters
    /// ----------
    /// value : str
    ///     Discount curve identifier.
    ///
    /// Returns
    /// -------
    /// CDSTrancheBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn discount_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        tranche_set!(
            slf,
            discount_curve_id,
            format!("{value:?}"),
            |b: CdsTrancheBuilderInner| b.discount_curve_id(CurveId::new(value.to_string()))
        )
    }

    /// Set the credit index identifier for survival/loss modeling.
    ///
    /// Parameters
    /// ----------
    /// value : str
    ///     Credit index identifier.
    ///
    /// Returns
    /// -------
    /// CDSTrancheBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn credit_index_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        tranche_set!(
            slf,
            credit_index_id,
            format!("{value:?}"),
            |b: CdsTrancheBuilderInner| b.credit_index_id(CurveId::new(value.to_string()))
        )
    }

    /// Set the tranche side (``"pay"`` buys protection, ``"receive"`` sells it).
    ///
    /// Parameters
    /// ----------
    /// value : {"pay", "receive"}
    ///     Tranche side: ``"pay"`` buys protection, ``"receive"`` sells it.
    ///
    /// Returns
    /// -------
    /// CDSTrancheBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` is not a recognized side.
    #[pyo3(text_signature = "($self, value)")]
    fn side<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let side: PayReceive = enum_from_str(value, "side")?;
        tranche_set!(
            slf,
            side,
            format!("{value:?}"),
            |b: CdsTrancheBuilderInner| b.side(side)
        )
    }

    /// Set the contract start (effective) date for schedule anchoring.
    ///
    /// Parameters
    /// ----------
    /// value : datetime.date | str
    ///     Effective date. If never set, uses the as-of date (or the prior CDS
    ///     roll date when ``roll_rule`` is ``RollRule.CDS_IMM``).
    ///
    /// Returns
    /// -------
    /// CDSTrancheBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn start_date<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let start_date = extract_date(value)?;
        tranche_set!(
            slf,
            start_date,
            date_repr(start_date),
            |b: CdsTrancheBuilderInner| b.start_date(start_date)
        )
    }

    /// Set the realized (settled) loss.
    ///
    /// Parameters
    /// ----------
    /// value : float
    ///     Realized loss as a fraction of the original portfolio
    ///     notional. Defaults to ``0.0`` when never set explicitly.
    ///
    /// Returns
    /// -------
    /// CDSTrancheBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn realized_loss<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: f64,
    ) -> PyResult<PyRefMut<'py, Self>> {
        tranche_set!(
            slf,
            realized_loss,
            float_repr(value),
            |b: CdsTrancheBuilderInner| b.realized_loss(value)
        )
    }

    /// Set the coupon roll-date grid.
    ///
    /// Parameters
    /// ----------
    /// value : RollRule
    ///     ``RollRule.CDS_IMM`` for the standard CDS roll dates (20th of Mar,
    ///     Jun, Sep, Dec) or ``RollRule.NONE`` (the default) for a schedule
    ///     generated from ``frequency`` and ``stub``. ``RollRule.IMM`` is
    ///     rejected by ``build()``.
    ///
    /// Returns
    /// -------
    /// CDSTrancheBuilder
    ///     ``self``, for chaining.
    #[pyo3(text_signature = "($self, value)")]
    fn roll_rule<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: PyRef<'_, PyRollRule>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let rule = value.inner;
        tranche_set!(
            slf,
            roll_rule,
            format!("{rule:?}"),
            |b: CdsTrancheBuilderInner| b.roll_rule(rule)
        )
    }

    /// Set the stub convention for a bespoke coupon schedule.
    ///
    /// Parameters
    /// ----------
    /// value : str | StubKind
    ///     Stub rule used when ``roll_rule`` is ``RollRule.NONE``. Defaults to
    ///     ``"short_front"``.
    ///
    /// Returns
    /// -------
    /// CDSTrancheBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` is not a known stub rule.
    #[pyo3(text_signature = "($self, value)")]
    fn stub<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let stub = stub_kind_from_py(Some(value), "stub")?;
        tranche_set!(
            slf,
            stub,
            format!("{stub:?}"),
            |b: CdsTrancheBuilderInner| b.stub(stub)
        )
    }

    /// Set the upfront payment.
    ///
    /// Parameters
    /// ----------
    /// value : tuple[datetime.date | str, Money]
    ///     ``(payment_date, amount)``; the amount currency must match the
    ///     tranche notional.
    ///
    /// Returns
    /// -------
    /// CDSTrancheBuilder
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
        tranche_set!(slf, upfront, shown, |b: CdsTrancheBuilderInner| b
            .upfront((date, money)))
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
    /// CDSTrancheBuilder
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
        tranche_set!(slf, attributes, shown, |b: CdsTrancheBuilderInner| b
            .attributes(attrs))
    }

    /// Build the validated CDS tranche.
    ///
    /// Validation is the Rust ``CDSTranche::builder().build()`` invariants
    /// only; there is no additional binding-side check.
    ///
    /// Returns
    /// -------
    /// CDSTranche
    ///     The validated CDS tranche.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the builder was already consumed, a required field is missing,
    ///     or the completed tranche fails validation (``attach_pct >=
    ///     detach_pct``, fractional attach/detach, loss outside ``[0, 1]``).
    #[pyo3(text_signature = "($self)")]
    fn build(mut slf: PyRefMut<'_, Self>) -> PyResult<PyCDSTranche> {
        let b = take_builder(&mut slf.inner)?;
        let inner = b.build().map_err(core_to_py)?;
        Ok(PyCDSTranche { inner })
    }

    /// Return ``repr(self)`` listing the fields set so far.
    fn __repr__(&self) -> String {
        builder_repr("CDSTrancheBuilder", &self.fields)
    }
}

/// Register the tranche helper classes on the instruments submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyCDSTrancheParams>()?;
    Ok(())
}
