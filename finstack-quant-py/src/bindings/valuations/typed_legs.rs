//! Typed leg-spec wrappers (`FixedLegSpec`, `FloatLegSpec`, `PremiumLegSpec`,
//! `ProtectionLegSpec`) shared by the typed `InterestRateSwap`, `Swaption`,
//! `CreditDefaultSwap` and `CDSIndex` builders.
//!
//! Thin frozen wrappers: construction and validation stay in Rust, the
//! bindings only coerce Python inputs (`float | Rate`, `float | Bps`,
//! date-likes, serde strings) and expose the serde surface (`to_json`,
//! `from_json`, pickle) plus one getter per field.

use pyo3::prelude::*;
use rust_decimal::prelude::ToPrimitive;

use crate::bindings::core::dates::daycount::PyDayCount;
use crate::bindings::core::dates::schedule::PyStubKind;
use crate::bindings::core::dates::tenor::PyTenor;
use crate::bindings::date_utils::{date_to_py, extract_date};
use crate::bindings::pandas_utils::serde_to_py;
use crate::errors::{core_to_py, display_to_py};

use super::convert::{bps_from_py, enum_to_py_string, opt_repr, rate_decimal_from_py};
use super::instruments::{decimal_from_f64, enum_from_str, stub_kind_from_py};

type FloatingLegCompounding =
    finstack_quant_valuations::instruments::rates::irs::FloatingLegCompounding;

/// Coerce `str | dict` to a `FloatingLegCompounding`.
///
/// A bare string names a unit variant (`"simple"`); a dict carries a struct
/// variant such as `{"compounded_in_arrears": {"lookback_days": 0}}`.
pub(crate) fn compounding_from_py(
    py: Python<'_>,
    obj: &Bound<'_, PyAny>,
) -> PyResult<FloatingLegCompounding> {
    if let Ok(name) = obj.extract::<&str>() {
        return enum_from_str(name, "compounding");
    }
    crate::bindings::module_utils::py_to_serde(py, obj, "compounding")
}

/// Render a Decimal field as a Python float literal.
fn decimal_repr(value: rust_decimal::Decimal) -> String {
    value.to_string()
}

/// Typed wrapper for the Rust `FixedLegSpec` (fixed leg of an IRS/swaption).
///
/// Immutable; every constructor argument is readable back through a property
/// of the same name, and the wire form round-trips via ``to_json`` /
/// ``from_json`` (which also backs ``pickle``).
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "FixedLegSpec",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyFixedLegSpec {
    /// Inner canonical Rust fixed-leg spec.
    pub(crate) inner: finstack_quant_valuations::instruments::FixedLegSpec,
}

impl PyFixedLegSpec {
    /// Wrap a Rust fixed-leg spec.
    pub(crate) fn from_inner(inner: finstack_quant_valuations::instruments::FixedLegSpec) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyFixedLegSpec {
    /// Fixed leg of an interest-rate swap.
    ///
    /// Parameters
    /// ----------
    /// discount_curve_id : str
    ///     Discount curve identifier for pricing this leg.
    /// rate : float | Rate
    ///     Fixed rate as a decimal (``0.04`` = 4%) or a ``Rate``.
    /// frequency : Tenor
    ///     Payment frequency.
    /// day_count : DayCount
    ///     Day count convention for accrual.
    /// start : datetime.date | datetime.datetime | pandas.Timestamp | str
    ///     Start date of the fixed leg (ISO 8601 strings accepted).
    /// end : datetime.date | datetime.datetime | pandas.Timestamp | str
    ///     End date of the fixed leg (ISO 8601 strings accepted).
    /// compounding_simple : bool
    ///     If true, use simple interest on the accrual fraction. Required:
    ///     the canonical Rust ``FixedLegSpec`` field has no default.
    /// business_day_convention : str, default "modified_following"
    ///     Business day convention for payment dates.
    /// calendar_id : str, optional
    ///     Calendar used for business day adjustments.
    /// stub : str | StubKind, default "short_front"
    ///     Stub period handling rule.
    /// par_method : str, optional
    ///     Par-rate method override: ``"forward_based"`` or
    ///     ``"discount_ratio"``. ``None`` keeps the pricer default.
    /// payment_lag_days : int, default 0
    ///     Payment lag in business days after period end.
    /// end_of_month : bool, default False
    ///     End-of-month roll convention.
    ///
    /// Returns
    /// -------
    /// FixedLegSpec
    ///     The validated fixed-leg specification.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If an enum value is invalid, ``rate`` is not finite, or the accrual
    ///     period is malformed (``start >= end``).
    /// TypeError
    ///     If ``rate`` is neither a number nor a ``Rate``, or a date cannot be
    ///     interpreted.
    ///
    /// Examples
    /// --------
    /// >>> from finstack_quant.core.dates import DayCount, Tenor
    /// >>> from finstack_quant.valuations.instruments import FixedLegSpec
    /// >>> leg = FixedLegSpec(
    /// ...     "USD-OIS", 0.04, Tenor.semi_annual(), DayCount.THIRTY_360,
    /// ...     "2024-01-15", "2029-01-15", compounding_simple=False,
    /// ... )
    /// >>> leg.rate
    /// 0.04
    #[new]
    #[pyo3(signature = (discount_curve_id, rate, frequency, day_count, start, end, *,
                        compounding_simple, business_day_convention = "modified_following", calendar_id = None,
                        stub = None, par_method = None, payment_lag_days = 0, end_of_month = false))]
    #[pyo3(
        text_signature = "(discount_curve_id, rate, frequency, day_count, start, end, *, \
compounding_simple, business_day_convention='modified_following', calendar_id=None, stub='short_front', \
par_method=None, payment_lag_days=0, end_of_month=False)"
    )]
    // PyO3 binding: the argument list mirrors the Python keyword-argument API.
    #[allow(clippy::too_many_arguments)]
    fn new(
        discount_curve_id: &str,
        rate: &Bound<'_, PyAny>,
        frequency: PyRef<'_, PyTenor>,
        day_count: PyRef<'_, PyDayCount>,
        start: &Bound<'_, PyAny>,
        end: &Bound<'_, PyAny>,
        compounding_simple: bool,
        business_day_convention: &str,
        calendar_id: Option<String>,
        stub: Option<&Bound<'_, PyAny>>,
        par_method: Option<&str>,
        payment_lag_days: i32,
        end_of_month: bool,
    ) -> PyResult<Self> {
        let rate = rate_decimal_from_py(rate, "rate")?;
        let inner = finstack_quant_valuations::instruments::FixedLegSpec {
            discount_curve_id: finstack_quant_core::types::CurveId::new(
                discount_curve_id.to_string(),
            ),
            rate: decimal_from_f64(rate, "rate")?,
            frequency: frequency.inner,
            day_count: day_count.inner,
            business_day_convention: enum_from_str(
                business_day_convention,
                "business_day_convention",
            )?,
            calendar_id,
            stub: stub_kind_from_py(stub, "stub")?,
            start: extract_date(start)?,
            end: extract_date(end)?,
            par_method: par_method
                .map(|value| enum_from_str(value, "par_method"))
                .transpose()?,
            compounding_simple,
            payment_lag_days,
            end_of_month,
        };
        inner.validate().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Deserialize a fixed-leg spec from its serde JSON object.
    ///
    /// Parameters
    /// ----------
    /// json : str
    ///     JSON object with the same fields as the Rust ``FixedLegSpec``
    ///     (the ``fixed`` sub-object of an ``interest_rate_swap`` envelope).
    ///
    /// Returns
    /// -------
    /// FixedLegSpec
    ///     The validated leg.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the JSON is malformed, has unknown fields, or ``start >= end``.
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: finstack_quant_valuations::instruments::FixedLegSpec =
            serde_json::from_str(json).map_err(display_to_py)?;
        inner.validate().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    /// Serialize to the serde JSON object accepted by ``FixedLegSpec.from_json``.
    ///
    /// Returns
    /// -------
    /// str
    ///     JSON object (not an instrument envelope).
    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Serde form of the leg as a Python ``dict``.
    #[pyo3(text_signature = "($self)")]
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner)
    }

    /// Discount curve identifier.
    #[getter]
    fn discount_curve_id(&self) -> String {
        self.inner.discount_curve_id.to_string()
    }

    /// Fixed rate as a decimal (``0.04`` = 4%).
    #[getter]
    fn rate(&self) -> f64 {
        self.inner.rate.to_f64().unwrap_or(f64::NAN)
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

    /// Business day convention (serde string, e.g. ``"modified_following"``).
    #[getter]
    fn business_day_convention(&self) -> PyResult<String> {
        enum_to_py_string(&self.inner.business_day_convention)
    }

    /// Payment calendar identifier, or ``None``.
    #[getter]
    fn calendar_id(&self) -> Option<String> {
        self.inner.calendar_id.clone()
    }

    /// Stub rule.
    #[getter]
    fn stub(&self) -> PyStubKind {
        PyStubKind::from_inner(self.inner.stub)
    }

    /// Accrual start date.
    #[getter]
    fn start<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.start)
    }

    /// Accrual end date.
    #[getter]
    fn end<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.end)
    }

    /// Par-rate method override (``"forward_based"`` / ``"discount_ratio"``) or ``None``.
    #[getter]
    fn par_method(&self) -> PyResult<Option<String>> {
        self.inner
            .par_method
            .as_ref()
            .map(enum_to_py_string)
            .transpose()
    }

    /// Whether simple interest is used on the accrual fraction.
    #[getter]
    fn compounding_simple(&self) -> bool {
        self.inner.compounding_simple
    }

    /// Payment lag in business days after period end.
    #[getter]
    fn payment_lag_days(&self) -> i32 {
        self.inner.payment_lag_days
    }

    /// End-of-month roll convention flag.
    #[getter]
    fn end_of_month(&self) -> bool {
        self.inner.end_of_month
    }

    /// Return ``repr(self)``.
    pub(crate) fn __repr__(&self) -> String {
        format!(
            "FixedLegSpec(discount_curve_id={:?}, rate={}, frequency={}, day_count={}, start={}, end={}, \
compounding_simple={}, business_day_convention={:?}, calendar_id={}, stub={:?}, par_method={}, \
payment_lag_days={}, end_of_month={})",
            self.inner.discount_curve_id.as_str(),
            decimal_repr(self.inner.rate),
            self.inner.frequency,
            self.inner.day_count,
            self.inner.start,
            self.inner.end,
            super::convert::bool_repr(self.inner.compounding_simple),
            enum_to_py_string(&self.inner.business_day_convention).unwrap_or_default(),
            opt_repr(self.inner.calendar_id.as_ref().map(|c| format!("{c:?}"))),
            enum_to_py_string(&self.inner.stub).unwrap_or_default(),
            opt_repr(
                self.inner
                    .par_method
                    .as_ref()
                    .map(|m| format!("{:?}", enum_to_py_string(m).unwrap_or_default()))
            ),
            self.inner.payment_lag_days,
            super::convert::bool_repr(self.inner.end_of_month),
        )
    }
}

/// Typed wrapper for the Rust `FloatLegSpec` (floating leg of an IRS/swaption).
///
/// Immutable; every constructor argument is readable back through a property
/// of the same name, and the wire form round-trips via ``to_json`` /
/// ``from_json`` (which also backs ``pickle``).
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "FloatLegSpec",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyFloatLegSpec {
    /// Inner canonical Rust floating-leg spec.
    pub(crate) inner: finstack_quant_valuations::instruments::FloatLegSpec,
}

impl PyFloatLegSpec {
    /// Wrap a Rust floating-leg spec.
    pub(crate) fn from_inner(inner: finstack_quant_valuations::instruments::FloatLegSpec) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyFloatLegSpec {
    /// Floating leg of an interest-rate swap.
    ///
    /// Parameters
    /// ----------
    /// discount_curve_id : str
    ///     Discount curve identifier for pricing this leg.
    /// forward_curve_id : str
    ///     Forward curve identifier for rate projections.
    /// spread_bp : float | Bps
    ///     Spread over the index in basis points (``25.0`` = 25bp) or a ``Bps``.
    /// frequency : Tenor
    ///     Payment frequency.
    /// day_count : DayCount
    ///     Day count convention for accrual.
    /// start : datetime.date | datetime.datetime | pandas.Timestamp | str
    ///     Start date of the floating leg (ISO 8601 strings accepted).
    /// end : datetime.date | datetime.datetime | pandas.Timestamp | str
    ///     End date of the floating leg (ISO 8601 strings accepted).
    /// business_day_convention : str, default "modified_following"
    ///     Business day convention for payment dates.
    /// calendar_id : str, optional
    ///     Calendar used for business day adjustments.
    /// stub : str | StubKind, default "short_front"
    ///     Stub period handling rule.
    /// reset_lag_days : int, default 0
    ///     Reset lag in business days before each accrual start. ``0`` (the
    ///     Rust default) fixes on the accrual start date, so a swap whose
    ///     first period starts on or after the valuation date prices off the
    ///     forward curve without historical fixings. Use ``2`` for a T-2
    ///     term index; ``InterestRateSwap.from_conventions`` applies the
    ///     registered market default for an index.
    /// fixing_calendar_id : str, optional
    ///     Calendar used for rate fixing (reset lag).
    /// compounding : str | dict, default "simple"
    ///     Coupon compounding. ``"simple"`` for term indices (LIBOR-style);
    ///     for overnight RFR legs pass a struct variant, e.g.
    ///     ``{"compounded_in_arrears": {"lookback_days": 0}}``,
    ///     ``{"compounded_with_observation_shift": {"shift_days": 0}}`` or
    ///     ``{"compounded_with_rate_cutoff": {"cutoff_days": 0}}``.
    /// payment_lag_days : int, default 0
    ///     Payment lag in business days after period end.
    /// end_of_month : bool, default False
    ///     End-of-month roll convention.
    ///
    /// Returns
    /// -------
    /// FloatLegSpec
    ///     The validated floating-leg specification.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If an enum value is invalid, ``compounding`` does not name a
    ///     variant, ``spread_bp`` is not finite, or the accrual period is
    ///     malformed (``start >= end``).
    /// TypeError
    ///     If ``spread_bp`` is neither a number nor a ``Bps``, or a date
    ///     cannot be interpreted.
    ///
    /// Examples
    /// --------
    /// >>> from finstack_quant.core.dates import DayCount, Tenor
    /// >>> from finstack_quant.valuations.instruments import FloatLegSpec
    /// >>> leg = FloatLegSpec(
    /// ...     "USD-OIS", "USD-SOFR-3M", 0.0, Tenor.quarterly(), DayCount.ACT_360,
    /// ...     "2024-01-15", "2029-01-15",
    /// ... )
    /// >>> leg.reset_lag_days
    /// 0
    #[new]
    #[pyo3(signature = (discount_curve_id, forward_curve_id, spread_bp, frequency, day_count,
                        start, end, *, business_day_convention = "modified_following", calendar_id = None,
                        stub = None, reset_lag_days = 0, fixing_calendar_id = None, compounding = None,
                        payment_lag_days = 0, end_of_month = false))]
    #[pyo3(
        text_signature = "(discount_curve_id, forward_curve_id, spread_bp, frequency, \
day_count, start, end, *, business_day_convention='modified_following', calendar_id=None, stub='short_front', \
reset_lag_days=0, fixing_calendar_id=None, compounding='simple', payment_lag_days=0, end_of_month=False)"
    )]
    // PyO3 binding: the argument list mirrors the Python keyword-argument API.
    #[allow(clippy::too_many_arguments)]
    fn new(
        py: Python<'_>,
        discount_curve_id: &str,
        forward_curve_id: &str,
        spread_bp: &Bound<'_, PyAny>,
        frequency: PyRef<'_, PyTenor>,
        day_count: PyRef<'_, PyDayCount>,
        start: &Bound<'_, PyAny>,
        end: &Bound<'_, PyAny>,
        business_day_convention: &str,
        calendar_id: Option<String>,
        stub: Option<&Bound<'_, PyAny>>,
        reset_lag_days: i32,
        fixing_calendar_id: Option<String>,
        compounding: Option<&Bound<'_, PyAny>>,
        payment_lag_days: i32,
        end_of_month: bool,
    ) -> PyResult<Self> {
        let spread_bp = bps_from_py(spread_bp, "spread_bp")?;
        let compounding = match compounding {
            Some(obj) if !obj.is_none() => compounding_from_py(py, obj)?,
            _ => FloatingLegCompounding::default(),
        };
        let inner = finstack_quant_valuations::instruments::FloatLegSpec {
            discount_curve_id: finstack_quant_core::types::CurveId::new(
                discount_curve_id.to_string(),
            ),
            forward_curve_id: finstack_quant_core::types::CurveId::new(
                forward_curve_id.to_string(),
            ),
            spread_bp: decimal_from_f64(spread_bp, "spread_bp")?,
            frequency: frequency.inner,
            day_count: day_count.inner,
            business_day_convention: enum_from_str(
                business_day_convention,
                "business_day_convention",
            )?,
            calendar_id,
            stub: stub_kind_from_py(stub, "stub")?,
            reset_lag_days,
            fixing_calendar_id,
            start: extract_date(start)?,
            end: extract_date(end)?,
            compounding,
            payment_lag_days,
            end_of_month,
        };
        inner.validate().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Deserialize a floating-leg spec from its serde JSON object.
    ///
    /// Parameters
    /// ----------
    /// json : str
    ///     JSON object with the same fields as the Rust ``FloatLegSpec``
    ///     (the ``float`` sub-object of an ``interest_rate_swap`` envelope).
    ///
    /// Returns
    /// -------
    /// FloatLegSpec
    ///     The validated leg.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the JSON is malformed, has unknown fields, or ``start >= end``.
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: finstack_quant_valuations::instruments::FloatLegSpec =
            serde_json::from_str(json).map_err(display_to_py)?;
        inner.validate().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    /// Serialize to the serde JSON object accepted by ``FloatLegSpec.from_json``.
    ///
    /// Returns
    /// -------
    /// str
    ///     JSON object (not an instrument envelope).
    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Serde form of the leg as a Python ``dict``.
    #[pyo3(text_signature = "($self)")]
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner)
    }

    /// Discount curve identifier.
    #[getter]
    fn discount_curve_id(&self) -> String {
        self.inner.discount_curve_id.to_string()
    }

    /// Forward (projection) curve identifier.
    #[getter]
    fn forward_curve_id(&self) -> String {
        self.inner.forward_curve_id.to_string()
    }

    /// Spread over the index in basis points.
    #[getter]
    fn spread_bp(&self) -> f64 {
        self.inner.spread_bp.to_f64().unwrap_or(f64::NAN)
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

    /// Business day convention (serde string).
    #[getter]
    fn business_day_convention(&self) -> PyResult<String> {
        enum_to_py_string(&self.inner.business_day_convention)
    }

    /// Payment calendar identifier, or ``None``.
    #[getter]
    fn calendar_id(&self) -> Option<String> {
        self.inner.calendar_id.clone()
    }

    /// Stub rule.
    #[getter]
    fn stub(&self) -> PyStubKind {
        PyStubKind::from_inner(self.inner.stub)
    }

    /// Reset lag in business days before each accrual start.
    #[getter]
    fn reset_lag_days(&self) -> i32 {
        self.inner.reset_lag_days
    }

    /// Fixing calendar identifier, or ``None``.
    #[getter]
    fn fixing_calendar_id(&self) -> Option<String> {
        self.inner.fixing_calendar_id.clone()
    }

    /// Accrual start date.
    #[getter]
    fn start<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.start)
    }

    /// Accrual end date.
    #[getter]
    fn end<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.end)
    }

    /// Coupon compounding in serde form: ``"simple"`` or a one-key ``dict``
    /// for the compounded variants.
    #[getter]
    fn compounding<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.compounding)
    }

    /// Payment lag in business days after period end.
    #[getter]
    fn payment_lag_days(&self) -> i32 {
        self.inner.payment_lag_days
    }

    /// End-of-month roll convention flag.
    #[getter]
    fn end_of_month(&self) -> bool {
        self.inner.end_of_month
    }

    /// Return ``repr(self)``.
    pub(crate) fn __repr__(&self) -> String {
        let compounding = serde_json::to_value(&self.inner.compounding)
            .map(|v| match v {
                serde_json::Value::String(s) => format!("{s:?}"),
                other => other.to_string(),
            })
            .unwrap_or_default();
        format!(
            "FloatLegSpec(discount_curve_id={:?}, forward_curve_id={:?}, spread_bp={}, frequency={}, \
day_count={}, start={}, end={}, business_day_convention={:?}, calendar_id={}, stub={:?}, \
reset_lag_days={}, fixing_calendar_id={}, compounding={}, payment_lag_days={}, end_of_month={})",
            self.inner.discount_curve_id.as_str(),
            self.inner.forward_curve_id.as_str(),
            decimal_repr(self.inner.spread_bp),
            self.inner.frequency,
            self.inner.day_count,
            self.inner.start,
            self.inner.end,
            enum_to_py_string(&self.inner.business_day_convention).unwrap_or_default(),
            opt_repr(self.inner.calendar_id.as_ref().map(|c| format!("{c:?}"))),
            enum_to_py_string(&self.inner.stub).unwrap_or_default(),
            self.inner.reset_lag_days,
            opt_repr(
                self.inner
                    .fixing_calendar_id
                    .as_ref()
                    .map(|c| format!("{c:?}"))
            ),
            compounding,
            self.inner.payment_lag_days,
            super::convert::bool_repr(self.inner.end_of_month),
        )
    }
}

/// Typed wrapper for the Rust `PremiumLegSpec` (CDS/CDS-index premium leg).
///
/// Immutable; fields are readable through properties and the wire form
/// round-trips via ``to_json`` / ``from_json`` (which also backs ``pickle``).
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "PremiumLegSpec",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyPremiumLegSpec {
    /// Inner canonical Rust premium-leg spec.
    pub(crate) inner: finstack_quant_valuations::instruments::PremiumLegSpec,
}

#[pymethods]
impl PyPremiumLegSpec {
    /// Premium (fixed coupon) leg of a CDS or CDS index.
    ///
    /// Parameters
    /// ----------
    /// start : datetime.date | datetime.datetime | pandas.Timestamp | str
    ///     Start date of protection / premium accrual.
    /// end : datetime.date | datetime.datetime | pandas.Timestamp | str
    ///     End date of protection / premium accrual.
    /// frequency : Tenor
    ///     Payment frequency.
    /// day_count : DayCount
    ///     Day count convention for accrual.
    /// spread_bp : float | Bps
    ///     Fixed running spread in basis points (``100.0`` = 100bp = 1%).
    /// discount_curve_id : str
    ///     Discount curve identifier for pricing this leg.
    /// standard_imm_dates : bool
    ///     True for prescribed quarterly CDS 20th dates; false for bespoke
    ///     frequency/stub schedules. Standard dates require quarterly frequency
    ///     and a short-front stub when the leg is priced.
    /// stub : str | StubKind, default "short_front"
    ///     Stub period handling rule.
    /// business_day_convention : str, default "modified_following"
    ///     Business day convention for payment dates.
    /// calendar_id : str, optional
    ///     Calendar used for business day adjustments.
    ///
    /// Returns
    /// -------
    /// PremiumLegSpec
    ///     The premium-leg specification.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If an enum value is invalid or ``spread_bp`` is not finite.
    /// TypeError
    ///     If ``spread_bp`` is neither a number nor a ``Bps``, or a date
    ///     cannot be interpreted.
    ///
    /// Examples
    /// --------
    /// >>> from finstack_quant.core.dates import DayCount, Tenor
    /// >>> from finstack_quant.valuations.instruments import PremiumLegSpec
    /// >>> leg = PremiumLegSpec(
    /// ...     "2024-03-20", "2029-06-20", Tenor.quarterly(), DayCount.ACT_360, 100.0, "USD-OIS",
    /// ...     standard_imm_dates=True,
    /// ... )
    /// >>> leg.spread_bp
    /// 100.0
    #[new]
    #[pyo3(signature = (start, end, frequency, day_count, spread_bp, discount_curve_id, *, standard_imm_dates,
                        stub = None, business_day_convention = "modified_following", calendar_id = None))]
    #[pyo3(
        text_signature = "(start, end, frequency, day_count, spread_bp, discount_curve_id, *, standard_imm_dates, \
stub='short_front', business_day_convention='modified_following', calendar_id=None)"
    )]
    // PyO3 binding: the argument list mirrors the Python keyword-argument API.
    #[allow(clippy::too_many_arguments)]
    fn new(
        start: &Bound<'_, PyAny>,
        end: &Bound<'_, PyAny>,
        frequency: PyRef<'_, PyTenor>,
        day_count: PyRef<'_, PyDayCount>,
        spread_bp: &Bound<'_, PyAny>,
        discount_curve_id: &str,
        standard_imm_dates: bool,
        stub: Option<&Bound<'_, PyAny>>,
        business_day_convention: &str,
        calendar_id: Option<String>,
    ) -> PyResult<Self> {
        let spread_bp = bps_from_py(spread_bp, "spread_bp")?;
        let inner = finstack_quant_valuations::instruments::PremiumLegSpec {
            standard_imm_dates,
            start: extract_date(start)?,
            end: extract_date(end)?,
            frequency: frequency.inner,
            stub: stub_kind_from_py(stub, "stub")?,
            business_day_convention: enum_from_str(
                business_day_convention,
                "business_day_convention",
            )?,
            calendar_id,
            day_count: day_count.inner,
            spread_bp: decimal_from_f64(spread_bp, "spread_bp")?,
            discount_curve_id: finstack_quant_core::types::CurveId::new(
                discount_curve_id.to_string(),
            ),
        };
        Ok(Self { inner })
    }

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Deserialize a premium-leg spec from its serde JSON object.
    ///
    /// Parameters
    /// ----------
    /// json : str
    ///     JSON object with the same fields as the Rust ``PremiumLegSpec``.
    ///
    /// Returns
    /// -------
    /// PremiumLegSpec
    ///     The leg.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the JSON is malformed or has unknown fields.
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: finstack_quant_valuations::instruments::PremiumLegSpec =
            serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }

    /// Serialize to the serde JSON object accepted by ``PremiumLegSpec.from_json``.
    ///
    /// Returns
    /// -------
    /// str
    ///     JSON object (not an instrument envelope).
    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Serde form of the leg as a Python ``dict``.
    #[pyo3(text_signature = "($self)")]
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner)
    }

    /// Whether the leg uses prescribed quarterly CDS 20th dates.
    #[getter]
    fn standard_imm_dates(&self) -> bool {
        self.inner.standard_imm_dates
    }

    /// Protection / accrual start date.
    #[getter]
    fn start<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.start)
    }

    /// Protection / accrual end date.
    #[getter]
    fn end<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.end)
    }

    /// Payment frequency.
    #[getter]
    fn frequency(&self) -> PyTenor {
        PyTenor::from_inner(self.inner.frequency)
    }

    /// Stub rule.
    #[getter]
    fn stub(&self) -> PyStubKind {
        PyStubKind::from_inner(self.inner.stub)
    }

    /// Business day convention (serde string).
    #[getter]
    fn business_day_convention(&self) -> PyResult<String> {
        enum_to_py_string(&self.inner.business_day_convention)
    }

    /// Payment calendar identifier, or ``None``.
    #[getter]
    fn calendar_id(&self) -> Option<String> {
        self.inner.calendar_id.clone()
    }

    /// Accrual day-count convention.
    #[getter]
    fn day_count(&self) -> PyDayCount {
        PyDayCount::from_inner(self.inner.day_count)
    }

    /// Running spread in basis points.
    #[getter]
    fn spread_bp(&self) -> f64 {
        self.inner.spread_bp.to_f64().unwrap_or(f64::NAN)
    }

    /// Discount curve identifier.
    #[getter]
    fn discount_curve_id(&self) -> String {
        self.inner.discount_curve_id.to_string()
    }

    /// Return ``repr(self)``.
    pub(crate) fn __repr__(&self) -> String {
        format!(
            "PremiumLegSpec(start={}, end={}, frequency={}, day_count={}, spread_bp={}, \
discount_curve_id={:?}, stub={:?}, business_day_convention={:?}, calendar_id={})",
            self.inner.start,
            self.inner.end,
            self.inner.frequency,
            self.inner.day_count,
            decimal_repr(self.inner.spread_bp),
            self.inner.discount_curve_id.as_str(),
            enum_to_py_string(&self.inner.stub).unwrap_or_default(),
            enum_to_py_string(&self.inner.business_day_convention).unwrap_or_default(),
            opt_repr(self.inner.calendar_id.as_ref().map(|c| format!("{c:?}"))),
        )
    }
}

/// Typed wrapper for the Rust `ProtectionLegSpec` (CDS/CDS-index protection leg).
///
/// Immutable; fields are readable through properties and the wire form
/// round-trips via ``to_json`` / ``from_json`` (which also backs ``pickle``).
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "ProtectionLegSpec",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyProtectionLegSpec {
    /// Inner canonical Rust protection-leg spec.
    pub(crate) inner: finstack_quant_valuations::instruments::ProtectionLegSpec,
}

#[pymethods]
impl PyProtectionLegSpec {
    /// Protection (default-contingent) leg of a CDS or CDS index.
    ///
    /// Parameters
    /// ----------
    /// credit_curve_id : str
    ///     Hazard/credit curve identifier for default probabilities.
    /// recovery_rate : float
    ///     Recovery rate in ``[0.0, 1.0]`` (e.g. 0.4 = 40%).
    /// settlement_delay : int, default 3
    ///     Settlement delay in business days.
    ///
    /// Returns
    /// -------
    /// ProtectionLegSpec
    ///     The validated protection-leg specification.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``recovery_rate`` is outside ``[0.0, 1.0]``.
    ///
    /// Examples
    /// --------
    /// >>> from finstack_quant.valuations.instruments import ProtectionLegSpec
    /// >>> leg = ProtectionLegSpec("ACME-CDS", 0.4, 3)
    /// >>> leg.recovery_rate
    /// 0.4
    #[new]
    #[pyo3(signature = (credit_curve_id, recovery_rate, settlement_delay = 3))]
    #[pyo3(text_signature = "(credit_curve_id, recovery_rate, settlement_delay=3)")]
    fn new(credit_curve_id: &str, recovery_rate: f64, settlement_delay: u16) -> PyResult<Self> {
        let inner = finstack_quant_valuations::instruments::ProtectionLegSpec::new(
            credit_curve_id.to_string(),
            recovery_rate,
            settlement_delay,
        )
        .map_err(core_to_py)?;
        Ok(Self { inner })
    }

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Deserialize a protection-leg spec from its serde JSON object.
    ///
    /// Parameters
    /// ----------
    /// json : str
    ///     JSON object with the same fields as the Rust ``ProtectionLegSpec``.
    ///
    /// Returns
    /// -------
    /// ProtectionLegSpec
    ///     The validated leg.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the JSON is malformed, has unknown fields, or the recovery rate
    ///     is outside ``[0.0, 1.0]``.
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: finstack_quant_valuations::instruments::ProtectionLegSpec =
            serde_json::from_str(json).map_err(display_to_py)?;
        finstack_quant_valuations::instruments::ProtectionLegSpec::validate_recovery_rate(
            inner.recovery_rate,
        )
        .map_err(core_to_py)?;
        Ok(Self { inner })
    }

    /// Serialize to the serde JSON object accepted by ``ProtectionLegSpec.from_json``.
    ///
    /// Returns
    /// -------
    /// str
    ///     JSON object (not an instrument envelope).
    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Serde form of the leg as a Python ``dict``.
    #[pyo3(text_signature = "($self)")]
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner)
    }

    /// Hazard / credit curve identifier.
    #[getter]
    fn credit_curve_id(&self) -> String {
        self.inner.credit_curve_id.to_string()
    }

    /// Recovery rate as a decimal in ``[0.0, 1.0]``.
    #[getter]
    fn recovery_rate(&self) -> f64 {
        self.inner.recovery_rate
    }

    /// Settlement delay in business days.
    #[getter]
    fn settlement_delay(&self) -> u16 {
        self.inner.settlement_delay
    }

    /// Return ``repr(self)``.
    pub(crate) fn __repr__(&self) -> String {
        format!(
            "ProtectionLegSpec(credit_curve_id={:?}, recovery_rate={}, settlement_delay={})",
            self.inner.credit_curve_id.as_str(),
            self.inner.recovery_rate,
            self.inner.settlement_delay
        )
    }
}

/// Register the leg-spec classes on the instruments submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyFixedLegSpec>()?;
    m.add_class::<PyFloatLegSpec>()?;
    m.add_class::<PyPremiumLegSpec>()?;
    m.add_class::<PyProtectionLegSpec>()?;
    Ok(())
}

/// Names this module contributes to `finstack_quant.valuations.instruments.__all__`.
///
/// Extend this list (sorted) when adding a class or function here; `mod.rs`
/// merges every submodule list so registration stays in one place per file.
pub(crate) const EXPORTS: &[&str] = &[];
