//! CapFloor Python bindings.

use super::swap::decimal_f64;
use pyo3::prelude::*;

use crate::bindings::core::dates::daycount::PyDayCount;
use crate::bindings::core::dates::schedule::PyStubKind;
use crate::bindings::core::dates::tenor::PyTenor;
use crate::bindings::core::money::PyMoney;
use crate::bindings::date_utils::{date_to_py, py_to_date};
use crate::bindings::module_utils::spec_from_py;
use crate::errors::core_to_py;
use finstack_quant_core::types::{CalendarId, CurveId, InstrumentId};

use super::super::convert::{attributes_from_py, enum_to_py_string, money_from_py, money_to_py};
use crate::bindings::extract::extract_rate_decimal;
use crate::bindings::valuations::convert::money_repr;
use crate::bindings::valuations::convert::{
    decimal_from_f64, enum_from_str, instrument_expiry, opt_serde_to_py, stub_kind_from_py,
};
use crate::bindings::valuations::typed_macros::{
    builder_set, instrument_envelope_methods, instrument_pricing_methods,
};

use finstack_quant_valuations::instruments::rates::cap_floor::CapFloorBuilder;

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.cap_floor", "CapFloor")]
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "CapFloor",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyCapFloor {
    /// Inner canonical Rust cap/floor.
    pub(crate) inner: finstack_quant_valuations::instruments::rates::cap_floor::CapFloor,
}

instrument_envelope_methods!(
    PyCapFloor(
        "CapFloor",
        module = "finstack_quant.valuations.instruments._typed_rates.cap_floor"
    ),
    CapFloor,
    builder = PyCapFloorBuilder("CapFloorBuilder", fields),
    seed = finstack_quant_valuations::instruments::rates::cap_floor::CapFloor::builder(),
);
instrument_pricing_methods!(
    PyCapFloor,
    "CapFloor",
    module = "finstack_quant.valuations.instruments._typed_rates.cap_floor"
);

#[pymethods]
impl PyCapFloor {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.cap_floor", "CapFloor.example")]
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn example() -> PyResult<Self> {
        finstack_quant_valuations::instruments::rates::cap_floor::CapFloor::example()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.cap_floor", "CapFloor.start_date")]
    #[getter]
    fn start_date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.start_date)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.cap_floor", "CapFloor.maturity")]
    #[getter]
    fn maturity<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.maturity)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.cap_floor", "CapFloor.overnight_coupon")]
    #[getter]
    fn overnight_coupon<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        opt_serde_to_py(py, self.inner.overnight_coupon.as_ref())
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.cap_floor", "CapFloor.premium")]
    #[getter]
    fn premium<'py>(&self, py: Python<'py>) -> PyResult<Option<(Bound<'py, PyAny>, PyMoney)>> {
        self.inner
            .premium
            .map(|(date, amount)| Ok((date_to_py(py, date)?, money_to_py(amount))))
            .transpose()
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.cap_floor", "CapFloor.expiry")]
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        instrument_expiry(py, &self.inner)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.cap_floor", "CapFloor.to_builder")]
    fn to_builder(&self) -> PyCapFloorBuilder {
        PyCapFloorBuilder {
            inner: Some(self.inner.to_builder()),
            fields: Vec::new(),
        }
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "CapFloor(id={:?}, rate_option_type={:?}, notional={}, strike={}, start_date={}, maturity={}, forward_curve_id={:?}, vol_surface_id={:?})",
            self.inner.id.as_str(),
            enum_to_py_string(&self.inner.rate_option_type).unwrap_or_default(),
            money_repr(self.inner.notional),
            self.inner.strike,
            self.inner.start_date,
            self.inner.maturity,
            self.inner.forward_curve_id.as_str(),
            self.inner.vol_surface_id.as_str(),
        )
    }
}

crate::bindings::macros::py_getters!(PyCapFloor |s| {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.cap_floor", "CapFloor.rate_option_type")]
    rate_option_type -> PyResult<String> = enum_to_py_string(&s.inner.rate_option_type);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.cap_floor", "CapFloor.notional")]
    notional -> PyMoney = money_to_py(s.inner.notional);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.cap_floor", "CapFloor.strike")]
    strike -> f64 = decimal_f64(s.inner.strike);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.cap_floor", "CapFloor.spread_bp")]
    spread_bp -> f64 = decimal_f64(s.inner.spread_bp);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.cap_floor", "CapFloor.frequency")]
    frequency -> PyTenor = PyTenor::from_inner(s.inner.frequency);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.cap_floor", "CapFloor.day_count")]
    day_count -> PyDayCount = PyDayCount::from_inner(s.inner.day_count);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.cap_floor", "CapFloor.stub")]
    stub -> PyStubKind = PyStubKind::from_inner(s.inner.stub);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.cap_floor", "CapFloor.business_day_convention")]
    business_day_convention -> PyResult<String> =
        enum_to_py_string(&s.inner.business_day_convention);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.cap_floor", "CapFloor.calendar_id")]
    calendar_id -> Option<String> = s.inner.calendar_id.as_ref().map(ToString::to_string);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.cap_floor", "CapFloor.exercise_style")]
    exercise_style -> PyResult<String> = enum_to_py_string(&s.inner.exercise_style);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.cap_floor", "CapFloor.settlement")]
    settlement -> PyResult<String> = enum_to_py_string(&s.inner.settlement);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.cap_floor", "CapFloor.discount_curve_id")]
    discount_curve_id -> String = s.inner.discount_curve_id.to_string();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.cap_floor", "CapFloor.forward_curve_id")]
    forward_curve_id -> String = s.inner.forward_curve_id.to_string();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.cap_floor", "CapFloor.vol_surface_id")]
    vol_surface_id -> String = s.inner.vol_surface_id.to_string();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.cap_floor", "CapFloor.vol_type")]
    vol_type -> PyResult<String> = enum_to_py_string(&s.inner.vol_type);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.cap_floor", "CapFloor.vol_shift")]
    vol_shift -> f64 = s.inner.vol_shift;
});

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.cap_floor", "CapFloorBuilder")]
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "CapFloorBuilder",
    skip_from_py_object
)]
pub struct PyCapFloorBuilder {
    inner: Option<CapFloorBuilder>,
    fields: Vec<(&'static str, String)>,
}

#[pymethods]
impl PyCapFloorBuilder {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.cap_floor", "CapFloorBuilder.id")]
    #[pyo3(text_signature = "($self, value)")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, id, format!("{value:?}"), |b: CapFloorBuilder| b
            .id(InstrumentId::new(value.to_string())))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.cap_floor", "CapFloorBuilder.rate_option_type")]
    #[pyo3(text_signature = "($self, value)")]
    fn rate_option_type<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let rate_option_type = enum_from_str(value, "rate_option_type")?;
        builder_set!(
            slf,
            rate_option_type,
            format!("{value:?}"),
            |b: CapFloorBuilder| b.rate_option_type(rate_option_type)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.cap_floor", "CapFloorBuilder.notional")]
    #[pyo3(text_signature = "($self, value)")]
    fn notional<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let money = money_from_py(value, "notional")?;
        builder_set!(slf, notional, money_repr(money), |b: CapFloorBuilder| b
            .notional(money))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.cap_floor", "CapFloorBuilder.strike")]
    #[pyo3(text_signature = "($self, value)")]
    fn strike<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let strike = extract_rate_decimal(value, "strike")?;
        let strike = decimal_from_f64(strike, "strike")?;
        builder_set!(slf, strike, strike.to_string(), |b: CapFloorBuilder| b
            .strike(strike))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.cap_floor", "CapFloorBuilder.spread_bp")]
    #[pyo3(text_signature = "($self, value)")]
    fn spread_bp<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let spread_bp = crate::bindings::extract::extract_basis_points(value, "spread_bp")?;
        let spread_bp = decimal_from_f64(spread_bp, "spread_bp")?;
        builder_set!(
            slf,
            spread_bp,
            spread_bp.to_string(),
            |b: CapFloorBuilder| b.spread_bp(spread_bp)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.cap_floor", "CapFloorBuilder.premium")]
    #[pyo3(text_signature = "($self, payment_date, amount)")]
    fn premium<'py>(
        mut slf: PyRefMut<'py, Self>,
        payment_date: &Bound<'_, PyAny>,
        amount: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let payment_date = py_to_date(payment_date)?;
        let amount = money_from_py(amount, "amount")?;
        builder_set!(
            slf,
            premium,
            format!("({payment_date}, {})", money_repr(amount)),
            |b: CapFloorBuilder| b.premium((payment_date, amount))
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.cap_floor", "CapFloorBuilder.start_date")]
    #[pyo3(text_signature = "($self, value)")]
    fn start_date<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let start_date = py_to_date(value)?;
        builder_set!(
            slf,
            start_date,
            start_date.to_string(),
            |b: CapFloorBuilder| b.start_date(start_date)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.cap_floor", "CapFloorBuilder.maturity")]
    #[pyo3(text_signature = "($self, value)")]
    fn maturity<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let maturity = py_to_date(value)?;
        builder_set!(slf, maturity, maturity.to_string(), |b: CapFloorBuilder| b
            .maturity(maturity))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.cap_floor", "CapFloorBuilder.frequency")]
    #[pyo3(text_signature = "($self, value)")]
    fn frequency<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let tenor = crate::bindings::core::dates::tenor::extract_tenor(value)?;
        builder_set!(slf, frequency, tenor.to_string(), |b: CapFloorBuilder| b
            .frequency(tenor))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.cap_floor", "CapFloorBuilder.day_count")]
    #[pyo3(text_signature = "($self, value)")]
    fn day_count<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let day_count =
            crate::bindings::valuations::convert::day_count_from_py(value, "day_count")?;
        builder_set!(
            slf,
            day_count,
            day_count.to_string(),
            |b: CapFloorBuilder| b.day_count(day_count)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.cap_floor", "CapFloorBuilder.stub")]
    #[pyo3(text_signature = "($self, value)")]
    fn stub<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let stub = stub_kind_from_py(Some(value), "stub")?;
        builder_set!(
            slf,
            stub,
            format!("{:?}", enum_to_py_string(&stub).unwrap_or_default()),
            |b: CapFloorBuilder| b.stub(stub)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.cap_floor", "CapFloorBuilder.business_day_convention")]
    #[pyo3(text_signature = "($self, value)")]
    fn business_day_convention<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let convention = super::super::convert::bdc_from_str(value, "business_day_convention")?;
        builder_set!(
            slf,
            business_day_convention,
            format!("{value:?}"),
            |b: CapFloorBuilder| b.business_day_convention(convention)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.cap_floor", "CapFloorBuilder.calendar_id")]
    #[pyo3(text_signature = "($self, value)")]
    fn calendar_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(
            slf,
            calendar_id,
            format!("{value:?}"),
            |b: CapFloorBuilder| b.calendar_id(CalendarId::new(value.to_string()))
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.cap_floor", "CapFloorBuilder.exercise_style")]
    #[pyo3(text_signature = "($self, value)")]
    fn exercise_style<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let style = enum_from_str(value, "exercise_style")?;
        builder_set!(
            slf,
            exercise_style,
            format!("{value:?}"),
            |b: CapFloorBuilder| b.exercise_style(style)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.cap_floor", "CapFloorBuilder.settlement")]
    #[pyo3(text_signature = "($self, value)")]
    fn settlement<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let settlement = enum_from_str(value, "settlement")?;
        builder_set!(
            slf,
            settlement,
            format!("{value:?}"),
            |b: CapFloorBuilder| b.settlement(settlement)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.cap_floor", "CapFloorBuilder.discount_curve_id")]
    #[pyo3(text_signature = "($self, value)")]
    fn discount_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(
            slf,
            discount_curve_id,
            format!("{value:?}"),
            |b: CapFloorBuilder| b.discount_curve_id(CurveId::new(value.to_string()))
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.cap_floor", "CapFloorBuilder.forward_curve_id")]
    #[pyo3(text_signature = "($self, value)")]
    fn forward_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(
            slf,
            forward_curve_id,
            format!("{value:?}"),
            |b: CapFloorBuilder| b.forward_curve_id(CurveId::new(value.to_string()))
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.cap_floor", "CapFloorBuilder.vol_surface_id")]
    #[pyo3(text_signature = "($self, value)")]
    fn vol_surface_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(
            slf,
            vol_surface_id,
            format!("{value:?}"),
            |b: CapFloorBuilder| b.vol_surface_id(CurveId::new(value.to_string()))
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.cap_floor", "CapFloorBuilder.vol_type")]
    #[pyo3(text_signature = "($self, value)")]
    fn vol_type<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let vol_type = enum_from_str(value, "vol_type")?;
        builder_set!(slf, vol_type, format!("{value:?}"), |b: CapFloorBuilder| b
            .vol_type(vol_type))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.cap_floor", "CapFloorBuilder.vol_shift")]
    #[pyo3(text_signature = "($self, value)")]
    fn vol_shift<'py>(mut slf: PyRefMut<'py, Self>, value: f64) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, vol_shift, value.to_string(), |b: CapFloorBuilder| b
            .vol_shift(value))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.cap_floor", "CapFloorBuilder.overnight_coupon")]
    #[pyo3(text_signature = "($self, value)")]
    fn overnight_coupon<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'py>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let convention: finstack_quant_valuations::instruments::rates::cap_floor::OvernightCouponConvention =
            spec_from_py(py, value, "overnight_coupon")?;
        builder_set!(
            slf,
            overnight_coupon,
            "{...}".to_string(),
            |b: CapFloorBuilder| b.overnight_coupon(convention)
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_rates.cap_floor", "CapFloorBuilder.attributes")]
    #[pyo3(text_signature = "($self, value)")]
    fn attributes<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let attrs = attributes_from_py(value)?;
        builder_set!(
            slf,
            attributes,
            "Attributes(...)".to_string(),
            |b: CapFloorBuilder| b.attributes(attrs)
        )
    }
}
