//! Typed leg-spec wrappers (`FixedLegSpec`, `FloatLegSpec`, `PremiumLegSpec`,
//! `ProtectionLegSpec`) shared by the typed `InterestRateSwap`, `Swaption`,
//! `CreditDefaultSwap` and `CdsIndex` builders.
//!
//! Thin frozen wrappers: construction and validation stay in Rust, the
//! bindings only coerce Python inputs (`float | Rate`, `float | Bps`,
//! date-likes, serde strings) and expose the serde surface (`to_json`,
//! `from_json`, pickle) plus one getter per field.

use pyo3::prelude::*;
use rust_decimal::prelude::ToPrimitive;

use crate::bindings::cashflows::builder::specs::PyRollRule;
use crate::bindings::core::dates::daycount::PyDayCount;
use crate::bindings::core::dates::schedule::PyStubKind;
use crate::bindings::core::dates::tenor::PyTenor;
use crate::bindings::date_utils::{date_to_py, py_to_date};
use crate::bindings::pandas_utils::serde_to_py;
use crate::errors::{core_to_py, display_to_py};

use super::convert::{decimal_from_f64, enum_from_str, stub_kind_from_py};
use super::convert::{enum_to_py_string, opt_repr};
use crate::bindings::extract::{extract_basis_points, extract_rate_decimal};

use finstack_quant_cashflows::builder::FloatingLegCompounding;

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

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_legs", "FixedLegSpec")]
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
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_legs", "FixedLegSpec.__init__")]
    #[new]
    #[pyo3(signature = (discount_curve_id, rate, frequency, day_count, start, end, *,
                        business_day_convention = "modified_following", calendar_id = None,
                        stub = None, payment_lag_days = 0, end_of_month = false))]
    #[pyo3(
        text_signature = "(discount_curve_id, rate, frequency, day_count, start, end, *, \
business_day_convention='modified_following', calendar_id=None, stub='short_front', \
payment_lag_days=0, end_of_month=False)"
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
        business_day_convention: &str,
        calendar_id: Option<String>,
        stub: Option<&Bound<'_, PyAny>>,
        payment_lag_days: i32,
        end_of_month: bool,
    ) -> PyResult<Self> {
        let rate = extract_rate_decimal(rate, "rate")?;
        let inner = finstack_quant_valuations::instruments::FixedLegSpec {
            discount_curve_id: finstack_quant_core::types::CurveId::new(
                discount_curve_id.to_string(),
            ),
            rate: decimal_from_f64(rate, "rate")?,
            frequency: frequency.inner,
            day_count: day_count.inner,
            business_day_convention: super::convert::bdc_from_str(
                business_day_convention,
                "business_day_convention",
            )?,
            calendar_id: calendar_id.map(Into::into),
            stub: stub_kind_from_py(stub, "stub")?,
            start: py_to_date(start)?,
            end: py_to_date(end)?,
            payment_lag_days,
            end_of_month,
        };
        inner.validate().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_legs", "FixedLegSpec.__reduce__")]
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_legs", "FixedLegSpec.from_json")]
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: finstack_quant_valuations::instruments::FixedLegSpec =
            serde_json::from_str(json).map_err(display_to_py)?;
        inner.validate().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_legs", "FixedLegSpec.to_json")]
    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_legs", "FixedLegSpec.to_dict")]
    #[pyo3(text_signature = "($self)")]
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_legs", "FixedLegSpec.start")]
    #[getter]
    fn start<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.start)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_legs", "FixedLegSpec.end")]
    #[getter]
    fn end<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.end)
    }

    /// Return ``repr(self)``.
    pub(crate) fn __repr__(&self) -> String {
        format!(
            "FixedLegSpec(discount_curve_id={:?}, rate={}, frequency={}, day_count={}, start={}, end={}, \
business_day_convention={:?}, calendar_id={}, stub={:?}, \
payment_lag_days={}, end_of_month={})",
            self.inner.discount_curve_id.as_str(),
            self.inner.rate,
            self.inner.frequency,
            self.inner.day_count,
            self.inner.start,
            self.inner.end,
            enum_to_py_string(&self.inner.business_day_convention).unwrap_or_default(),
            opt_repr(self.inner.calendar_id.as_ref().map(|c| format!("{c:?}"))),
            enum_to_py_string(&self.inner.stub).unwrap_or_default(),
            self.inner.payment_lag_days,
            super::convert::bool_repr(self.inner.end_of_month),
        )
    }
}

crate::bindings::macros::py_getters!(PyFixedLegSpec |s| {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_legs", "FixedLegSpec.discount_curve_id")]
    discount_curve_id -> String = s.inner.discount_curve_id.to_string();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_legs", "FixedLegSpec.rate")]
    rate -> f64 = s.inner.rate.to_f64().unwrap_or(f64::NAN);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_legs", "FixedLegSpec.frequency")]
    frequency -> PyTenor = PyTenor::from_inner(s.inner.frequency);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_legs", "FixedLegSpec.day_count")]
    day_count -> PyDayCount = PyDayCount::from_inner(s.inner.day_count);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_legs", "FixedLegSpec.business_day_convention")]
    business_day_convention -> PyResult<String> =
        enum_to_py_string(&s.inner.business_day_convention);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_legs", "FixedLegSpec.calendar_id")]
    calendar_id -> Option<String> = s.inner.calendar_id.as_ref().map(ToString::to_string);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_legs", "FixedLegSpec.stub")]
    stub -> PyStubKind = PyStubKind::from_inner(s.inner.stub);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_legs", "FixedLegSpec.payment_lag_days")]
    payment_lag_days -> i32 = s.inner.payment_lag_days;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_legs", "FixedLegSpec.end_of_month")]
    end_of_month -> bool = s.inner.end_of_month;
});

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_legs", "FloatLegSpec")]
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
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_legs", "FloatLegSpec.__init__")]
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
        let spread_bp = extract_basis_points(spread_bp, "spread_bp")?;
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
            business_day_convention: super::convert::bdc_from_str(
                business_day_convention,
                "business_day_convention",
            )?,
            calendar_id: calendar_id.map(Into::into),
            stub: stub_kind_from_py(stub, "stub")?,
            reset_lag_days,
            fixing_calendar_id: fixing_calendar_id.map(Into::into),
            start: py_to_date(start)?,
            end: py_to_date(end)?,
            compounding,
            payment_lag_days,
            end_of_month,
        };
        inner.validate().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_legs", "FloatLegSpec.__reduce__")]
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_legs", "FloatLegSpec.from_json")]
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: finstack_quant_valuations::instruments::FloatLegSpec =
            serde_json::from_str(json).map_err(display_to_py)?;
        inner.validate().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_legs", "FloatLegSpec.to_json")]
    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_legs", "FloatLegSpec.to_dict")]
    #[pyo3(text_signature = "($self)")]
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_legs", "FloatLegSpec.start")]
    #[getter]
    fn start<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.start)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_legs", "FloatLegSpec.end")]
    #[getter]
    fn end<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.end)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_legs", "FloatLegSpec.compounding")]
    #[getter]
    fn compounding<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.compounding)
    }

    /// Return ``repr(self)``.
    pub(crate) fn __repr__(&self) -> String {
        let compounding = serde_json::to_value(self.inner.compounding)
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
            self.inner.spread_bp,
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

crate::bindings::macros::py_getters!(PyFloatLegSpec |s| {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_legs", "FloatLegSpec.discount_curve_id")]
    discount_curve_id -> String = s.inner.discount_curve_id.to_string();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_legs", "FloatLegSpec.forward_curve_id")]
    forward_curve_id -> String = s.inner.forward_curve_id.to_string();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_legs", "FloatLegSpec.spread_bp")]
    spread_bp -> f64 = s.inner.spread_bp.to_f64().unwrap_or(f64::NAN);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_legs", "FloatLegSpec.frequency")]
    frequency -> PyTenor = PyTenor::from_inner(s.inner.frequency);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_legs", "FloatLegSpec.day_count")]
    day_count -> PyDayCount = PyDayCount::from_inner(s.inner.day_count);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_legs", "FloatLegSpec.business_day_convention")]
    business_day_convention -> PyResult<String> =
        enum_to_py_string(&s.inner.business_day_convention);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_legs", "FloatLegSpec.calendar_id")]
    calendar_id -> Option<String> = s.inner.calendar_id.as_ref().map(ToString::to_string);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_legs", "FloatLegSpec.stub")]
    stub -> PyStubKind = PyStubKind::from_inner(s.inner.stub);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_legs", "FloatLegSpec.reset_lag_days")]
    reset_lag_days -> i32 = s.inner.reset_lag_days;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_legs", "FloatLegSpec.fixing_calendar_id")]
    fixing_calendar_id -> Option<String> =
        s.inner.fixing_calendar_id.as_ref().map(ToString::to_string);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_legs", "FloatLegSpec.payment_lag_days")]
    payment_lag_days -> i32 = s.inner.payment_lag_days;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_legs", "FloatLegSpec.end_of_month")]
    end_of_month -> bool = s.inner.end_of_month;
});

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_legs", "PremiumLegSpec")]
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
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_legs", "PremiumLegSpec.__init__")]
    #[new]
    #[pyo3(signature = (start, end, frequency, day_count, coupon_bp, discount_curve_id, *, roll_rule,
                        stub = None, business_day_convention = "modified_following", calendar_id = None))]
    #[pyo3(
        text_signature = "(start, end, frequency, day_count, coupon_bp, discount_curve_id, *, roll_rule, \
stub='short_front', business_day_convention='modified_following', calendar_id=None)"
    )]
    // PyO3 binding: the argument list mirrors the Python keyword-argument API.
    #[allow(clippy::too_many_arguments)]
    fn new(
        start: &Bound<'_, PyAny>,
        end: &Bound<'_, PyAny>,
        frequency: PyRef<'_, PyTenor>,
        day_count: PyRef<'_, PyDayCount>,
        coupon_bp: &Bound<'_, PyAny>,
        discount_curve_id: &str,
        roll_rule: PyRef<'_, PyRollRule>,
        stub: Option<&Bound<'_, PyAny>>,
        business_day_convention: &str,
        calendar_id: Option<String>,
    ) -> PyResult<Self> {
        let coupon_bp = extract_basis_points(coupon_bp, "coupon_bp")?;
        let inner = finstack_quant_valuations::instruments::PremiumLegSpec {
            roll_rule: roll_rule.inner,
            start: py_to_date(start)?,
            end: py_to_date(end)?,
            frequency: frequency.inner,
            stub: stub_kind_from_py(stub, "stub")?,
            business_day_convention: super::convert::bdc_from_str(
                business_day_convention,
                "business_day_convention",
            )?,
            calendar_id: calendar_id.map(Into::into),
            day_count: day_count.inner,
            coupon_bp: decimal_from_f64(coupon_bp, "coupon_bp")?,
            discount_curve_id: finstack_quant_core::types::CurveId::new(
                discount_curve_id.to_string(),
            ),
        };
        Ok(Self { inner })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_legs", "PremiumLegSpec.__reduce__")]
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_legs", "PremiumLegSpec.from_json")]
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: finstack_quant_valuations::instruments::PremiumLegSpec =
            serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_legs", "PremiumLegSpec.to_json")]
    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_legs", "PremiumLegSpec.to_dict")]
    #[pyo3(text_signature = "($self)")]
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_legs", "PremiumLegSpec.start")]
    #[getter]
    fn start<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.start)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_legs", "PremiumLegSpec.end")]
    #[getter]
    fn end<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.end)
    }

    /// Return ``repr(self)``.
    pub(crate) fn __repr__(&self) -> String {
        format!(
            "PremiumLegSpec(start={}, end={}, frequency={}, day_count={}, coupon_bp={}, \
discount_curve_id={:?}, stub={:?}, business_day_convention={:?}, calendar_id={})",
            self.inner.start,
            self.inner.end,
            self.inner.frequency,
            self.inner.day_count,
            self.inner.coupon_bp,
            self.inner.discount_curve_id.as_str(),
            enum_to_py_string(&self.inner.stub).unwrap_or_default(),
            enum_to_py_string(&self.inner.business_day_convention).unwrap_or_default(),
            opt_repr(self.inner.calendar_id.as_ref().map(|c| format!("{c:?}"))),
        )
    }
}

crate::bindings::macros::py_getters!(PyPremiumLegSpec |s| {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_legs", "PremiumLegSpec.roll_rule")]
    roll_rule -> PyRollRule = PyRollRule { inner: s.inner.roll_rule };

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_legs", "PremiumLegSpec.frequency")]
    frequency -> PyTenor = PyTenor::from_inner(s.inner.frequency);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_legs", "PremiumLegSpec.stub")]
    stub -> PyStubKind = PyStubKind::from_inner(s.inner.stub);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_legs", "PremiumLegSpec.business_day_convention")]
    business_day_convention -> PyResult<String> =
        enum_to_py_string(&s.inner.business_day_convention);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_legs", "PremiumLegSpec.calendar_id")]
    calendar_id -> Option<String> = s.inner.calendar_id.as_ref().map(ToString::to_string);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_legs", "PremiumLegSpec.day_count")]
    day_count -> PyDayCount = PyDayCount::from_inner(s.inner.day_count);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_legs", "PremiumLegSpec.coupon_bp")]
    coupon_bp -> f64 = s.inner.coupon_bp.to_f64().unwrap_or(f64::NAN);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_legs", "PremiumLegSpec.discount_curve_id")]
    discount_curve_id -> String = s.inner.discount_curve_id.to_string();
});

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_legs", "ProtectionLegSpec")]
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
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_legs", "ProtectionLegSpec.__init__")]
    #[new]
    #[pyo3(text_signature = "(credit_curve_id, recovery_rate, settlement_delay)")]
    fn new(credit_curve_id: &str, recovery_rate: f64, settlement_delay: u16) -> PyResult<Self> {
        let inner = finstack_quant_valuations::instruments::ProtectionLegSpec::new(
            credit_curve_id.to_string(),
            recovery_rate,
            settlement_delay,
        )
        .map_err(core_to_py)?;
        Ok(Self { inner })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_legs", "ProtectionLegSpec.__reduce__")]
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_legs", "ProtectionLegSpec.from_json")]
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: finstack_quant_valuations::instruments::ProtectionLegSpec =
            serde_json::from_str(json).map_err(display_to_py)?;
        finstack_quant_core::validation::validate_recovery_rate(inner.recovery_rate)
            .map_err(core_to_py)?;
        Ok(Self { inner })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_legs", "ProtectionLegSpec.to_json")]
    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_legs", "ProtectionLegSpec.to_dict")]
    #[pyo3(text_signature = "($self)")]
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner)
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

crate::bindings::macros::py_getters!(PyProtectionLegSpec |s| {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_legs", "ProtectionLegSpec.credit_curve_id")]
    credit_curve_id -> String = s.inner.credit_curve_id.to_string();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_legs", "ProtectionLegSpec.recovery_rate")]
    recovery_rate -> f64 = s.inner.recovery_rate;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_legs", "ProtectionLegSpec.settlement_delay")]
    settlement_delay -> u16 = s.inner.settlement_delay;
});

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
pub(crate) const EXPORTS: &[&str] = &[
    "FixedLegSpec",
    "FloatLegSpec",
    "PremiumLegSpec",
    "ProtectionLegSpec",
];
