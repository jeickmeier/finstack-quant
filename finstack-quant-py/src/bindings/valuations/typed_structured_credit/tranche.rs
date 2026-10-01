use pyo3::prelude::*;

use crate::bindings::core::dates::daycount::PyDayCount;
use crate::bindings::core::dates::tenor::PyTenor;
use crate::bindings::core::money::PyMoney;
use crate::bindings::date_utils::{date_to_py, extract_date};
use crate::bindings::valuations::convert::{
    attributes_from_py, attributes_to_py, bool_repr, enum_to_py_string, money_to_py, opt_repr,
    rate_decimal_from_py,
};
use crate::errors::{core_to_py, value_error};
use finstack_quant_core::types::CreditRating;
use finstack_quant_valuations::instruments::fixed_income::loan_terms::RateSpec;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    CoverageTrigger, Tranche, TrancheSeniority,
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
            attach_pct: None,
            detach_pct: None,
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

    /// Attachment point in percent of the capital structure (0-100 scale),
    /// or ``None`` for a note built without points that has not yet joined
    /// a ``TrancheStructure`` (which derives it from the balance shares).
    #[getter]
    fn attach_pct(&self) -> Option<f64> {
        self.inner.attach_pct
    }

    /// Detachment point in percent of the capital structure (0-100 scale),
    /// or ``None`` until derived (see ``attach_pct``).
    #[getter]
    fn detach_pct(&self) -> Option<f64> {
        self.inner.detach_pct
    }

    /// Seniority (serde name, e.g. ``"Senior"``, ``"Mezzanine"``, ``"Equity"``).
    #[getter]
    fn seniority(&self) -> PyResult<String> {
        enum_to_py_string(&self.inner.seniority)
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

    /// Coupon definition as a plain ``dict`` (``RateSpec`` serde shape).
    #[getter]
    fn coupon<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::pandas_utils::serde_to_py(py, &self.inner.coupon)
    }

    /// Payment frequency.
    #[getter]
    fn frequency(&self) -> PyTenor {
        PyTenor::from_inner(self.inner.frequency)
    }

    /// Accrual day-count convention.
    #[getter]
    fn day_count(&self) -> PyDayCount {
        PyDayCount::from_inner(self.inner.day_count)
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

    /// Explicit non-deferrable flag, or ``None`` for the seniority
    /// convention (senior notes non-deferrable, every other class defers).
    #[getter]
    fn non_deferrable(&self) -> Option<bool> {
        self.inner.non_deferrable
    }

    /// Whether the coupon is a non-deferrable claim the template pays from
    /// principal proceeds when interest falls short.
    #[getter]
    fn is_non_deferrable(&self) -> bool {
        self.inner.is_non_deferrable()
    }

    /// Legal final maturity as ``datetime.date``.
    #[getter]
    fn maturity<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.maturity)
    }

    /// Per-tranche overcollateralization trigger as its ``CoverageTrigger``
    /// serde ``dict``, or ``None``.
    #[getter]
    fn oc_trigger<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .oc_trigger
            .as_ref()
            .map(|trigger| crate::bindings::pandas_utils::serde_to_py(py, trigger))
            .transpose()
    }

    /// Per-tranche interest-coverage trigger as its ``CoverageTrigger``
    /// serde ``dict``, or ``None``.
    #[getter]
    fn ic_trigger<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .ic_trigger
            .as_ref()
            .map(|trigger| crate::bindings::pandas_utils::serde_to_py(py, trigger))
            .transpose()
    }

    /// Payment priority rank (1 = most senior), assigned by
    /// ``TrancheStructure``; ``0`` on a standalone tranche.
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
            "Tranche(id='{}', seniority='{}', attach_pct={}, detach_pct={}, original_balance={}, maturity='{}', pik_enabled={})",
            self.inner.id.as_str(),
            enum_to_py_string(&self.inner.seniority).unwrap_or_default(),
            opt_repr(self.inner.attach_pct),
            opt_repr(self.inner.detach_pct),
            self.inner.original_balance.amount(),
            self.inner.maturity,
            bool_repr(self.inner.pik_enabled),
        )
    }
}

/// Fluent builder for [`PyTranche`]; wraps the hand-written Rust
/// `TrancheBuilder` (consuming setters).
///
/// ``attach_pct`` and ``detach_pct`` are forwarded to the Rust builder, which
/// requires both or neither on :meth:`build`; the copies kept here only feed
/// ``repr``.
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "TrancheBuilder",
    skip_from_py_object
)]
pub struct PyTrancheBuilder {
    inner: Option<TrancheBuilderInner>,
    attach_pct: Option<f64>,
    detach_pct: Option<f64>,
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
    fn attach_pct<'py>(mut slf: PyRefMut<'py, Self>, value: f64) -> PyResult<PyRefMut<'py, Self>> {
        let b = take_tranche(&mut slf)?;
        slf.inner = Some(b.attach_pct(value));
        slf.attach_pct = Some(value);
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
    fn detach_pct<'py>(mut slf: PyRefMut<'py, Self>, value: f64) -> PyResult<PyRefMut<'py, Self>> {
        let b = take_tranche(&mut slf)?;
        slf.inner = Some(b.detach_pct(value));
        slf.detach_pct = Some(value);
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
        slf.inner = Some(b.coupon(RateSpec::Fixed { rate }));
        Ok(slf)
    }

    /// Set a floating-rate coupon from a JSON ``RateSpec::Floating`` payload.
    ///
    /// The floating-rate spec (``FloatingRateSpec``: index, spread, gearing,
    /// floors/caps, reset conventions) stays JSON per the nested-spec rule —
    /// the typed cashflows plan owns that shape.
    ///
    /// Parameters
    /// ----------
    /// value : str
    ///     JSON-encoded, externally-tagged ``RateSpec`` value, e.g.
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
    ///     If ``value`` is not valid JSON for the ``RateSpec`` shape.
    #[pyo3(text_signature = "($self, value)")]
    fn coupon_floating<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let coupon: RateSpec = crate::bindings::module_utils::py_to_serde(py, value, "coupon")?;
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
    /// value : Tenor | str
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
    ///     :meth:`TrancheBuilder.build`, or a string ``value`` is not a recognized tenor string.
    #[pyo3(text_signature = "($self, value)")]
    fn frequency<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let frequency = crate::bindings::valuations::convert::tenor_from_py(value, "frequency")?;
        let b = take_tranche(&mut slf)?;
        slf.inner = Some(b.frequency(frequency));
        Ok(slf)
    }

    /// Set the day count convention for interest accrual.
    ///
    /// Parameters
    /// ----------
    /// value : DayCount | str
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
    ///     :meth:`TrancheBuilder.build`, or a string ``value`` is not a recognized day-count name.
    #[pyo3(text_signature = "($self, value)")]
    fn day_count<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let day_count =
            crate::bindings::valuations::convert::day_count_from_py(value, "day_count")?;
        let b = take_tranche(&mut slf)?;
        slf.inner = Some(b.day_count(day_count));
        Ok(slf)
    }

    /// Set the current (factored) balance.
    ///
    /// Parameters
    /// ----------
    /// value : Money
    ///     Outstanding principal today, at most the original balance.
    ///     Defaults to the original balance when never set.
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
    fn current_balance<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: PyRef<'_, PyMoney>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let b = take_tranche(&mut slf)?;
        slf.inner = Some(b.current_balance(value.inner));
        Ok(slf)
    }

    /// Set interest already deferred (unpaid, still owed) at closing.
    ///
    /// Parameters
    /// ----------
    /// value : Money
    ///     Deferred interest carried into the projection; zero when never
    ///     set.
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
    fn deferred_interest<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: PyRef<'_, PyMoney>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let b = take_tranche(&mut slf)?;
        slf.inner = Some(b.deferred_interest(value.inner));
        Ok(slf)
    }

    /// Enable payment-in-kind accretion of interest shortfalls.
    ///
    /// Parameters
    /// ----------
    /// value : bool
    ///     ``True`` capitalizes unpaid interest into the balance; ``False``
    ///     (the default) defers it as a claim.
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
    fn pik_enabled<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: bool,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let b = take_tranche(&mut slf)?;
        slf.inner = Some(b.pik_enabled(value));
        Ok(slf)
    }

    /// Mark the coupon non-deferrable or deferrable, overriding the
    /// seniority convention.
    ///
    /// Parameters
    /// ----------
    /// value : bool
    ///     ``True`` for a coupon the template pays from principal proceeds
    ///     when interest falls short (the senior default); ``False`` for one
    ///     that defers (the default for every other class).
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
    fn non_deferrable<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: bool,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let b = take_tranche(&mut slf)?;
        slf.inner = Some(b.non_deferrable(value));
        Ok(slf)
    }

    /// Set the credit rating.
    ///
    /// Parameters
    /// ----------
    /// value : str
    ///     Rating string (``"AAA"``, ``"BBB"``, ``"NR"`` ...).
    ///
    /// Returns
    /// -------
    /// TrancheBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` is not a known rating or the builder was consumed.
    #[pyo3(text_signature = "($self, value)")]
    fn rating<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let rating: CreditRating = enum_from_str(value, "rating")?;
        let b = take_tranche(&mut slf)?;
        slf.inner = Some(b.rating(rating));
        Ok(slf)
    }

    /// Attach a per-tranche overcollateralization trigger.
    ///
    /// Parameters
    /// ----------
    /// value : dict | str
    ///     ``CoverageTrigger`` serde object: ``trigger_level`` (ratio),
    ///     optional ``cure_level``, ``consequence`` (``"divert_interest"``,
    ///     ``"pay_down_senior"`` ...) and breach memory fields.
    ///
    /// Returns
    /// -------
    /// TrancheBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` does not match the ``CoverageTrigger`` shape.
    #[pyo3(text_signature = "($self, value)")]
    fn oc_trigger<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let trigger: CoverageTrigger =
            crate::bindings::module_utils::py_to_serde(py, value, "oc_trigger")?;
        let b = take_tranche(&mut slf)?;
        slf.inner = Some(b.oc_trigger(trigger));
        Ok(slf)
    }

    /// Attach a per-tranche interest-coverage trigger.
    ///
    /// Parameters
    /// ----------
    /// value : dict | str
    ///     ``CoverageTrigger`` serde object (see :meth:`oc_trigger`).
    ///
    /// Returns
    /// -------
    /// TrancheBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` does not match the ``CoverageTrigger`` shape.
    #[pyo3(text_signature = "($self, value)")]
    fn ic_trigger<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let trigger: CoverageTrigger =
            crate::bindings::module_utils::py_to_serde(py, value, "ic_trigger")?;
        let b = take_tranche(&mut slf)?;
        slf.inner = Some(b.ic_trigger(trigger));
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
    /// TrancheBuilder
    ///     ``self``, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` is neither ``Attributes`` nor a string dict.
    #[pyo3(text_signature = "($self, value)")]
    fn attributes<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let attributes = attributes_from_py(value)?;
        let b = take_tranche(&mut slf)?;
        slf.inner = Some(b.attributes(attributes));
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
        let b = take_tranche(&mut slf)?;
        let inner = b.build().map_err(core_to_py)?;
        Ok(PyTranche { inner })
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "TrancheBuilder(attach_pct={}, detach_pct={}, consumed={})",
            opt_repr(self.attach_pct),
            opt_repr(self.detach_pct),
            bool_repr(self.inner.is_none()),
        )
    }
}
