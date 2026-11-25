//! Typed inflation-linked bond construction and pricing.

use crate::bindings::core::dates::daycount::{PyDayCount, extract_day_count};
use crate::bindings::core::dates::tenor::{PyTenor, extract_tenor};
use crate::bindings::core::money::{PyMoney, decimal_from_py, decimal_to_py};
use crate::bindings::date_utils::{date_to_py, py_to_date};
use crate::bindings::pydoc::pydoc;
use crate::errors::core_to_py;
use finstack_quant_core::market_data::scalars::InflationLag;
use finstack_quant_core::types::{CalendarId, CurveId, InstrumentId};
use finstack_quant_valuations::instruments::fixed_income::inflation_linked_bond::{
    InflationLinkedBond, InflationLinkedBondBuilder,
};
use pyo3::prelude::*;

use super::convert::{
    attributes_from_py, enum_from_str, enum_to_py_string, instrument_expiry, money_from_py,
};
use super::typed_macros::{builder_set, instrument_envelope_methods, instrument_pricing_methods};

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_inflation_bond", "InflationLinkedBond")]
#[pyclass(
    name = "InflationLinkedBond",
    module = "finstack_quant.valuations.instruments",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyInflationLinkedBond {
    pub(crate) inner: InflationLinkedBond,
}

instrument_envelope_methods!(
    PyInflationLinkedBond(
        "InflationLinkedBond",
        module = "finstack_quant.valuations.instruments._typed_inflation_bond"
    ),
    InflationLinkedBond,
    builder = PyInflationLinkedBondBuilder("InflationLinkedBondBuilder", no_fields),
    seed = InflationLinkedBond::builder(),
);
instrument_pricing_methods!(
    PyInflationLinkedBond,
    "InflationLinkedBond",
    module = "finstack_quant.valuations.instruments._typed_inflation_bond"
);

#[pymethods]
impl PyInflationLinkedBond {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_inflation_bond", "InflationLinkedBond.to_builder")]
    fn to_builder(&self) -> PyInflationLinkedBondBuilder {
        PyInflationLinkedBondBuilder {
            inner: Some(self.inner.to_builder()),
        }
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_inflation_bond", "InflationLinkedBond.example")]
    #[staticmethod]
    fn example() -> PyResult<Self> {
        InflationLinkedBond::example()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_inflation_bond", "InflationLinkedBond.expiry")]
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        instrument_expiry(py, &self.inner)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_inflation_bond", "InflationLinkedBond.issue_date")]
    #[getter]
    fn issue_date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.issue_date)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_inflation_bond", "InflationLinkedBond.maturity")]
    #[getter]
    fn maturity<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.maturity)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_inflation_bond", "InflationLinkedBond.real_coupon")]
    #[getter]
    fn real_coupon<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        decimal_to_py(py, self.inner.real_coupon)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_inflation_bond", "InflationLinkedBond.base_date")]
    #[getter]
    fn base_date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.base_date)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_inflation_bond", "InflationLinkedBond.indexation_method")]
    #[getter]
    fn indexation_method(&self) -> PyResult<String> {
        enum_to_py_string(&self.inner.indexation_method)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_inflation_bond", "InflationLinkedBond.deflation_protection")]
    #[getter]
    fn deflation_protection(&self) -> PyResult<String> {
        enum_to_py_string(&self.inner.deflation_protection)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_inflation_bond", "InflationLinkedBond.business_day_convention")]
    #[getter]
    fn business_day_convention(&self) -> PyResult<String> {
        enum_to_py_string(&self.inner.business_day_convention)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_inflation_bond", "InflationLinkedBond.stub")]
    #[getter]
    fn stub(&self) -> PyResult<String> {
        enum_to_py_string(&self.inner.stub)
    }
}

crate::bindings::macros::py_getters!(PyInflationLinkedBond |s| {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_inflation_bond", "InflationLinkedBond.notional")]
    notional -> PyMoney = PyMoney::from_inner(s.inner.notional);
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_inflation_bond", "InflationLinkedBond.frequency")]
    frequency -> PyTenor = PyTenor::from_inner(s.inner.frequency);
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_inflation_bond", "InflationLinkedBond.day_count")]
    day_count -> PyDayCount = PyDayCount::from_inner(s.inner.day_count);
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_inflation_bond", "InflationLinkedBond.base_cpi")]
    base_cpi -> f64 = s.inner.base_cpi;
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_inflation_bond", "InflationLinkedBond.lag")]
    lag -> String = s.inner.lag.to_string();
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_inflation_bond", "InflationLinkedBond.floor_vol_surface_id")]
    floor_vol_surface_id -> Option<String> = s.inner.floor_vol_surface_id.as_ref().map(ToString::to_string);
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_inflation_bond", "InflationLinkedBond.calendar_id")]
    calendar_id -> Option<String> = s.inner.calendar_id.as_ref().map(ToString::to_string);
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_inflation_bond", "InflationLinkedBond.discount_curve_id")]
    discount_curve_id -> String = s.inner.discount_curve_id.to_string();
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_inflation_bond", "InflationLinkedBond.inflation_index_id")]
    inflation_index_id -> String = s.inner.inflation_index_id.to_string();
});

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_inflation_bond", "InflationLinkedBondBuilder")]
#[pyclass(
    name = "InflationLinkedBondBuilder",
    module = "finstack_quant.valuations.instruments",
    skip_from_py_object
)]
pub struct PyInflationLinkedBondBuilder {
    inner: Option<InflationLinkedBondBuilder>,
}

#[pymethods]
impl PyInflationLinkedBondBuilder {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_inflation_bond", "InflationLinkedBondBuilder.id")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let value = InstrumentId::new(value);
        builder_set!(slf, |b: InflationLinkedBondBuilder| b.id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_inflation_bond", "InflationLinkedBondBuilder.notional")]
    fn notional<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = money_from_py(value, "notional")?;
        builder_set!(slf, |b: InflationLinkedBondBuilder| b.notional(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_inflation_bond", "InflationLinkedBondBuilder.issue_date")]
    fn issue_date<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_date(value)?;
        builder_set!(slf, |b: InflationLinkedBondBuilder| b.issue_date(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_inflation_bond", "InflationLinkedBondBuilder.maturity")]
    fn maturity<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_date(value)?;
        builder_set!(slf, |b: InflationLinkedBondBuilder| b.maturity(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_inflation_bond", "InflationLinkedBondBuilder.real_coupon")]
    fn real_coupon<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = decimal_from_py(value)?;
        builder_set!(slf, |b: InflationLinkedBondBuilder| b.real_coupon(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_inflation_bond", "InflationLinkedBondBuilder.frequency")]
    fn frequency<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = extract_tenor(value)?;
        builder_set!(slf, |b: InflationLinkedBondBuilder| b.frequency(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_inflation_bond", "InflationLinkedBondBuilder.day_count")]
    fn day_count<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = extract_day_count(value)?;
        builder_set!(slf, |b: InflationLinkedBondBuilder| b.day_count(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_inflation_bond", "InflationLinkedBondBuilder.base_date")]
    fn base_date<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_date(value)?;
        builder_set!(slf, |b: InflationLinkedBondBuilder| b.base_date(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_inflation_bond", "InflationLinkedBondBuilder.base_cpi")]
    fn base_cpi<'py>(mut slf: PyRefMut<'py, Self>, value: f64) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: InflationLinkedBondBuilder| b.base_cpi(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_inflation_bond", "InflationLinkedBondBuilder.indexation_method")]
    fn indexation_method<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = enum_from_str(value, "indexation_method")?;
        builder_set!(slf, |b: InflationLinkedBondBuilder| b
            .indexation_method(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_inflation_bond", "InflationLinkedBondBuilder.lag")]
    fn lag<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.parse::<InflationLag>().map_err(core_to_py)?;
        builder_set!(slf, |b: InflationLinkedBondBuilder| b.lag(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_inflation_bond", "InflationLinkedBondBuilder.deflation_protection")]
    fn deflation_protection<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = enum_from_str(value, "deflation_protection")?;
        builder_set!(slf, |b: InflationLinkedBondBuilder| b
            .deflation_protection(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_inflation_bond", "InflationLinkedBondBuilder.floor_vol_surface_id")]
    fn floor_vol_surface_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = CurveId::new(value);
        builder_set!(slf, |b: InflationLinkedBondBuilder| b
            .floor_vol_surface_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_inflation_bond", "InflationLinkedBondBuilder.business_day_convention")]
    fn business_day_convention<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = enum_from_str(value, "business_day_convention")?;
        builder_set!(slf, |b: InflationLinkedBondBuilder| b
            .business_day_convention(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_inflation_bond", "InflationLinkedBondBuilder.stub")]
    fn stub<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let value = enum_from_str(value, "stub")?;
        builder_set!(slf, |b: InflationLinkedBondBuilder| b.stub(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_inflation_bond", "InflationLinkedBondBuilder.calendar_id")]
    fn calendar_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = CalendarId::new(value);
        builder_set!(slf, |b: InflationLinkedBondBuilder| b.calendar_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_inflation_bond", "InflationLinkedBondBuilder.discount_curve_id")]
    fn discount_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = CurveId::new(value);
        builder_set!(slf, |b: InflationLinkedBondBuilder| b
            .discount_curve_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_inflation_bond", "InflationLinkedBondBuilder.inflation_index_id")]
    fn inflation_index_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = CurveId::new(value);
        builder_set!(slf, |b: InflationLinkedBondBuilder| b
            .inflation_index_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_inflation_bond", "InflationLinkedBondBuilder.attributes")]
    fn attributes<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = attributes_from_py(value)?;
        builder_set!(slf, |b: InflationLinkedBondBuilder| b.attributes(value))
    }
}

/// Register the inflation-linked bond and its builder.
pub fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyInflationLinkedBond>()?;
    module.add_class::<PyInflationLinkedBondBuilder>()?;
    Ok(())
}

/// Names this module contributes to `finstack_quant.valuations.instruments.__all__`.
pub(crate) const EXPORTS: &[&str] = &["InflationLinkedBond", "InflationLinkedBondBuilder"];
