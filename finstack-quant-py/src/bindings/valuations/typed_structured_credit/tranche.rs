use pyo3::prelude::*;

use crate::bindings::core::dates::daycount::PyDayCount;
use crate::bindings::core::dates::tenor::PyTenor;
use crate::bindings::core::money::PyMoney;
use crate::bindings::date_utils::{date_to_py, extract_date};
use crate::bindings::valuations::convert::{
    attributes_to_py, bool_repr, enum_to_py_string, money_to_py, opt_repr, rate_decimal_from_py,
};
use crate::errors::{core_to_py, value_error};
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    Tranche, TrancheCoupon, TrancheSeniority,
};

use super::super::instruments::enum_from_str;

type TrancheBuilderInner =
    finstack_quant_valuations::instruments::fixed_income::structured_credit::TrancheBuilder;

/// Typed wrapper for the Rust `Tranche`.
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "Tranche",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyTranche {
    /// Inner canonical Rust tranche.
    pub(crate) inner: Tranche,
}

#[pymethods]
impl PyTranche {
    /// Create a fluent builder (mirrors Rust ``Tranche::builder()``).
    ///
    /// Returns
    /// -------
    /// TrancheBuilder
    ///     A builder with fluent, consuming setter methods.
    ///
    /// Examples
    /// --------
    /// >>> from finstack_quant.valuations.instruments import Tranche
    /// >>> builder = Tranche.builder()
    /// >>> builder.id("EXAMPLE") is builder
    /// True
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn builder() -> PyTrancheBuilder {
        PyTrancheBuilder {
            inner: Some(Tranche::builder()),
            attachment_point: None,
            detachment_point: None,
        }
    }

    /// Deserialize from the JSON produced by ``to_json``.
    ///
    /// Parameters
    /// ----------
    /// json : str
    ///     Strict JSON object with exactly the fields ``to_json`` writes.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the JSON is malformed or has the wrong shape.
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner = serde_json::from_str(json)
            .map_err(|err| crate::errors::serde_json_to_py(err, "invalid Tranche JSON"))?;
        Ok(Self { inner })
    }

    /// Serialize to the canonical JSON wire form.
    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(crate::errors::display_to_py)
    }

    /// Return every field as a plain ``dict`` (canonical serde shape).
    #[pyo3(text_signature = "($self)")]
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::pandas_utils::serde_to_py(py, &self.inner)
    }

    /// Support ``pickle`` through the ``to_json`` / ``from_json`` round-trip.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Tranche identifier.
    #[getter]
    fn id(&self) -> String {
        self.inner.id.to_string()
    }

    /// Attachment point in percent of the capital structure (0-100 scale).
    #[getter]
    fn attachment_point(&self) -> f64 {
        self.inner.attachment_point
    }

    /// Detachment point in percent of the capital structure (0-100 scale).
    #[getter]
    fn detachment_point(&self) -> f64 {
        self.inner.detachment_point
    }

    /// Seniority (serde name, e.g. ``"Senior"``, ``"Mezzanine"``, ``"Equity"``).
    #[getter]
    fn seniority(&self) -> PyResult<String> {
        enum_to_py_string(&self.inner.seniority)
    }

    /// Behavior type (serde name).
    #[getter]
    fn behavior_type(&self) -> PyResult<String> {
        enum_to_py_string(&self.inner.behavior_type)
    }

    /// Credit rating (serde name) or ``None``.
    #[getter]
    fn rating(&self) -> PyResult<Option<String>> {
        self.inner
            .rating
            .as_ref()
            .map(enum_to_py_string)
            .transpose()
    }

    /// Original (issuance) balance.
    #[getter]
    fn original_balance(&self) -> PyMoney {
        money_to_py(self.inner.original_balance)
    }

    /// Current outstanding balance.
    #[getter]
    fn current_balance(&self) -> PyMoney {
        money_to_py(self.inner.current_balance)
    }

    /// Target balance for revolving structures, or ``None``.
    #[getter]
    fn target_balance(&self) -> Option<PyMoney> {
        self.inner.target_balance.map(money_to_py)
    }

    /// Coupon definition as a plain ``dict`` (``TrancheCoupon`` serde shape).
    #[getter]
    fn coupon<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::pandas_utils::serde_to_py(py, &self.inner.coupon)
    }

    /// Payment frequency (tenor string such as ``"3M"``).
    #[getter]
    fn frequency(&self) -> PyResult<String> {
        enum_to_py_string(&self.inner.frequency)
    }

    /// Accrual day count (serde name).
    #[getter]
    fn day_count(&self) -> PyResult<String> {
        enum_to_py_string(&self.inner.day_count)
    }

    /// Accumulated deferred (PIK) interest.
    #[getter]
    fn deferred_interest(&self) -> PyMoney {
        money_to_py(self.inner.deferred_interest)
    }

    /// Whether interest may be deferred (PIK).
    #[getter]
    fn pik_enabled(&self) -> bool {
        self.inner.pik_enabled
    }

    /// Whether the tranche balance revolves.
    #[getter]
    fn is_revolving(&self) -> bool {
        self.inner.is_revolving
    }

    /// Whether principal collections may be reinvested.
    #[getter]
    fn can_reinvest(&self) -> bool {
        self.inner.can_reinvest
    }

    /// Legal final maturity as ``datetime.date``.
    #[getter]
    fn maturity<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.maturity)
    }

    /// Payment priority rank (1 = most senior).
    #[getter]
    fn payment_priority(&self) -> u32 {
        self.inner.payment_priority
    }

    /// User attributes (tags and metadata).
    #[getter]
    fn attributes(&self) -> crate::bindings::core::types::PyAttributes {
        attributes_to_py(&self.inner.attributes)
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "Tranche(id='{}', seniority='{}', attachment_point={}, detachment_point={}, original_balance={}, maturity='{}', pik_enabled={})",
            self.inner.id.as_str(),
            enum_to_py_string(&self.inner.seniority).unwrap_or_default(),
            self.inner.attachment_point,
            self.inner.detachment_point,
            self.inner.original_balance.amount(),
            self.inner.maturity,
            bool_repr(self.inner.pik_enabled),
        )
    }
}

/// Fluent builder for [`PyTranche`]; wraps the hand-written Rust
/// `TrancheBuilder` (consuming setters).
///
/// ``attachment_point`` and ``detachment_point`` are tracked separately from
/// the wrapped Rust builder (which only exposes a combined
/// `attachment_detachment(a, d)` setter) and applied together on
/// :meth:`build`, so either call order works.
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "TrancheBuilder",
    skip_from_py_object
)]
pub struct PyTrancheBuilder {
    inner: Option<TrancheBuilderInner>,
    attachment_point: Option<f64>,
    detachment_point: Option<f64>,
}

/// Take the wrapped Rust builder or fail if `build()` already consumed it.
fn take_tranche(b: &mut PyTrancheBuilder) -> PyResult<TrancheBuilderInner> {
    b.inner
        .take()
        .ok_or_else(|| value_error("builder already consumed by build()"))
}

#[pymethods]
impl PyTrancheBuilder {
    /// Set the tranche identifier.
    ///
    /// Parameters
    /// ----------
    /// value : str
    ///     Unique identifier for the tranche.
    ///
    /// Returns
    /// -------
    /// TrancheBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If this builder was already consumed by a prior call to
    ///     :meth:`TrancheBuilder.build`.
    #[pyo3(text_signature = "($self, value)")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let b = take_tranche(&mut slf)?;
        slf.inner = Some(b.id(value));
        Ok(slf)
    }

    /// Set the attachment point.
    ///
    /// Parameters
    /// ----------
    /// value : float
    ///     Attachment point quoted in percent on a 0-100 scale (e.g. ``0.0``
    ///     for equity, ``10.0`` for a tranche attaching at 10%).
    ///
    /// Returns
    /// -------
    /// TrancheBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If this builder was already consumed by a prior call to
    ///     :meth:`TrancheBuilder.build`.
    #[pyo3(text_signature = "($self, value)")]
    fn attachment_point<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: f64,
    ) -> PyResult<PyRefMut<'py, Self>> {
        if slf.inner.is_none() {
            return Err(value_error("builder already consumed by build()"));
        }
        slf.attachment_point = Some(value);
        Ok(slf)
    }

    /// Set the detachment point.
    ///
    /// Parameters
    /// ----------
    /// value : float
    ///     Detachment point quoted in percent on a 0-100 scale (e.g.
    ///     ``100.0`` for the most senior tranche).
    ///
    /// Returns
    /// -------
    /// TrancheBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If this builder was already consumed by a prior call to
    ///     :meth:`TrancheBuilder.build`.
    #[pyo3(text_signature = "($self, value)")]
    fn detachment_point<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: f64,
    ) -> PyResult<PyRefMut<'py, Self>> {
        if slf.inner.is_none() {
            return Err(value_error("builder already consumed by build()"));
        }
        slf.detachment_point = Some(value);
        Ok(slf)
    }

    /// Set the tranche seniority.
    ///
    /// Parameters
    /// ----------
    /// value : {"senior", "mezzanine", "subordinated", "equity"}
    ///     Structural seniority of the tranche.
    ///
    /// Returns
    /// -------
    /// TrancheBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` is not a recognized seniority.
    #[pyo3(text_signature = "($self, value)")]
    fn seniority<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let seniority: TrancheSeniority = enum_from_str(value, "seniority")?;
        let b = take_tranche(&mut slf)?;
        slf.inner = Some(b.seniority(seniority));
        Ok(slf)
    }

    /// Set the original tranche balance.
    ///
    /// Maps to the Rust ``TrancheBuilder::balance`` setter; named
    /// ``original_balance`` here to match the ``Tranche::original_balance``
    /// field it populates.
    ///
    /// Parameters
    /// ----------
    /// value : Money
    ///     Original tranche balance. Must be positive.
    ///
    /// Returns
    /// -------
    /// TrancheBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If this builder was already consumed by a prior call to
    ///     :meth:`TrancheBuilder.build`.
    #[pyo3(text_signature = "($self, value)")]
    fn original_balance<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: PyRef<'_, PyMoney>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let b = take_tranche(&mut slf)?;
        slf.inner = Some(b.balance(value.inner));
        Ok(slf)
    }

    /// Set a fixed-rate coupon.
    ///
    /// Parameters
    /// ----------
    /// rate : float
    ///     Fixed interest rate as an annual decimal (e.g. ``0.05`` = 5%).
    ///
    /// Returns
    /// -------
    /// TrancheBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If this builder was already consumed by a prior call to
    ///     :meth:`TrancheBuilder.build`.
    #[pyo3(text_signature = "($self, rate)")]
    fn coupon_fixed<'py>(
        mut slf: PyRefMut<'py, Self>,
        rate: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let rate = rate_decimal_from_py(rate, "rate")?;
        let b = take_tranche(&mut slf)?;
        slf.inner = Some(b.coupon(TrancheCoupon::Fixed { rate }));
        Ok(slf)
    }

    /// Set a floating-rate coupon from a JSON ``TrancheCoupon::Floating`` payload.
    ///
    /// The floating-rate spec (``FloatingRateSpec``: index, spread, gearing,
    /// floors/caps, reset conventions) stays JSON per the nested-spec rule —
    /// the typed cashflows plan owns that shape.
    ///
    /// Parameters
    /// ----------
    /// value : str
    ///     JSON-encoded, externally-tagged ``TrancheCoupon`` value, e.g.
    ///     ``{"floating": {...FloatingRateSpec fields...}}``.
    ///
    /// Returns
    /// -------
    /// TrancheBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` is not valid JSON for the ``TrancheCoupon`` shape.
    #[pyo3(text_signature = "($self, value)")]
    fn coupon_floating<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let coupon: TrancheCoupon =
            crate::bindings::module_utils::py_to_serde(py, value, "coupon")?;
        let b = take_tranche(&mut slf)?;
        slf.inner = Some(b.coupon(coupon));
        Ok(slf)
    }

    /// Set the legal final maturity date.
    ///
    /// Parameters
    /// ----------
    /// value : datetime.date | datetime.datetime | pandas.Timestamp | str
    ///     Legal final maturity date (date-like or ISO-8601 string).
    ///
    /// Returns
    /// -------
    /// TrancheBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If this builder was already consumed by a prior call to
    ///     :meth:`TrancheBuilder.build`.
    #[pyo3(text_signature = "($self, value)")]
    fn maturity<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let maturity = extract_date(value)?;
        let b = take_tranche(&mut slf)?;
        slf.inner = Some(b.maturity(maturity));
        Ok(slf)
    }

    /// Set the payment frequency.
    ///
    /// Parameters
    /// ----------
    /// value : Tenor
    ///     Payment frequency. Defaults to quarterly when never set.
    ///
    /// Returns
    /// -------
    /// TrancheBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If this builder was already consumed by a prior call to
    ///     :meth:`TrancheBuilder.build`.
    #[pyo3(text_signature = "($self, value)")]
    fn frequency<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: PyRef<'_, PyTenor>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let b = take_tranche(&mut slf)?;
        slf.inner = Some(b.frequency(value.inner));
        Ok(slf)
    }

    /// Set the day count convention for interest accrual.
    ///
    /// Parameters
    /// ----------
    /// value : DayCount
    ///     Day count convention. Defaults to Act/360 when never set.
    ///
    /// Returns
    /// -------
    /// TrancheBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If this builder was already consumed by a prior call to
    ///     :meth:`TrancheBuilder.build`.
    #[pyo3(text_signature = "($self, value)")]
    fn day_count<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: PyRef<'_, PyDayCount>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let b = take_tranche(&mut slf)?;
        slf.inner = Some(b.day_count(value.inner));
        Ok(slf)
    }

    /// Build the validated tranche.
    ///
    /// Returns
    /// -------
    /// Tranche
    ///     The validated tranche.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a required field is missing, or attachment/detachment points
    ///     are invalid (negative, out of the ``[0, 100]`` range, or
    ///     detachment not strictly above attachment).
    #[pyo3(text_signature = "($self)")]
    fn build(mut slf: PyRefMut<'_, Self>) -> PyResult<PyTranche> {
        let mut b = take_tranche(&mut slf)?;
        if let (Some(attachment), Some(detachment)) = (slf.attachment_point, slf.detachment_point) {
            b = b.attachment_detachment(attachment, detachment);
        }
        let inner = b.build().map_err(core_to_py)?;
        Ok(PyTranche { inner })
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "TrancheBuilder(attachment_point={}, detachment_point={}, consumed={})",
            opt_repr(self.attachment_point),
            opt_repr(self.detachment_point),
            bool_repr(self.inner.is_none()),
        )
    }
}
