//! A structured note whose future coupons depend on prior coupon states.

use crate::bindings::core::money::PyMoney;
use crate::bindings::module_utils::py_to_serde;
use crate::bindings::pydoc::pydoc;
use crate::errors::core_to_py;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_valuations::instruments::exotics::snowball::{Snowball, SnowballBuilder};
use pyo3::prelude::*;

use super::convert::{attributes_from_py, instrument_expiry};
use super::typed_macros::{builder_set, instrument_envelope_methods, instrument_pricing_methods};

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_snowball", "Snowball")]
#[pyclass(
    name = "Snowball",
    module = "finstack_quant.valuations.instruments",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PySnowball {
    pub(crate) inner: Snowball,
}

instrument_envelope_methods!(
    PySnowball(
        "Snowball",
        module = "finstack_quant.valuations.instruments._typed_snowball"
    ),
    Snowball,
    builder = PySnowballBuilder("SnowballBuilder", no_fields),
    seed = Snowball::builder(),
);
instrument_pricing_methods!(
    PySnowball,
    "Snowball",
    module = "finstack_quant.valuations.instruments._typed_snowball"
);

#[pymethods]
impl PySnowball {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_snowball", "Snowball.to_builder")]
    fn to_builder(&self) -> PySnowballBuilder {
        PySnowballBuilder {
            inner: Some(self.inner.to_builder()),
        }
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_snowball", "Snowball.example")]
    #[staticmethod]
    fn example() -> PyResult<Self> {
        Snowball::example()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_snowball", "Snowball.expiry")]
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        instrument_expiry(py, &self.inner)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_snowball", "Snowball.variant")]
    #[getter]
    fn variant<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.variant)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_snowball", "Snowball.initial_coupon")]
    #[getter]
    fn initial_coupon(&self) -> f64 {
        self.inner.initial_coupon
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_snowball", "Snowball.fixed_rate")]
    #[getter]
    fn fixed_rate<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::core::money::decimal_to_py(py, self.inner.fixed_rate)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_snowball", "Snowball.gearing")]
    #[getter]
    fn gearing(&self) -> f64 {
        self.inner.gearing
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_snowball", "Snowball.coupon_floor")]
    #[getter]
    fn coupon_floor(&self) -> f64 {
        self.inner.coupon_floor
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_snowball", "Snowball.coupon_cap")]
    #[getter]
    fn coupon_cap(&self) -> Option<f64> {
        self.inner.coupon_cap
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_snowball", "Snowball.notional")]
    #[getter]
    fn notional(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.notional)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_snowball", "Snowball.start_date")]
    #[getter]
    fn start_date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::date_utils::date_to_py(py, self.inner.start_date)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_snowball", "Snowball.payment_dates")]
    #[getter]
    fn payment_dates<'py>(&self, py: Python<'py>) -> PyResult<Vec<Bound<'py, PyAny>>> {
        {
            let dates = &self.inner.payment_dates;
            dates
                .iter()
                .map(|date| crate::bindings::date_utils::date_to_py(py, *date))
                .collect::<PyResult<Vec<_>>>()
        }
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_snowball", "Snowball.forward_curve_id")]
    #[getter]
    fn forward_curve_id(&self) -> String {
        self.inner.forward_curve_id.to_string()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_snowball", "Snowball.index_tenor")]
    #[getter]
    fn index_tenor<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.index_tenor)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_snowball", "Snowball.discount_curve_id")]
    #[getter]
    fn discount_curve_id(&self) -> String {
        self.inner.discount_curve_id.to_string()
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_snowball", "Snowball.vol_surface_id")]
    #[getter]
    fn vol_surface_id(&self) -> Option<String> {
        self.inner.vol_surface_id.as_ref().map(ToString::to_string)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_snowball", "Snowball.call_provision")]
    #[getter]
    fn call_provision<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.call_provision)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_snowball", "Snowball.day_count")]
    #[getter]
    fn day_count(&self) -> crate::bindings::core::dates::daycount::PyDayCount {
        crate::bindings::core::dates::daycount::PyDayCount::from_inner(self.inner.day_count)
    }
}

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_snowball", "SnowballBuilder")]
#[pyclass(
    name = "SnowballBuilder",
    module = "finstack_quant.valuations.instruments",
    skip_from_py_object
)]
pub struct PySnowballBuilder {
    inner: Option<SnowballBuilder>,
}

#[pymethods]
impl PySnowballBuilder {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_snowball", "SnowballBuilder.id")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let value = InstrumentId::new(value);
        builder_set!(slf, |b: SnowballBuilder| b.id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_snowball", "SnowballBuilder.variant")]
    fn variant<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "variant")?;
        builder_set!(slf, |b: SnowballBuilder| b.variant(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_snowball", "SnowballBuilder.initial_coupon")]
    fn initial_coupon<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: f64,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: SnowballBuilder| b.initial_coupon(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_snowball", "SnowballBuilder.fixed_rate")]
    fn fixed_rate<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = crate::bindings::core::money::decimal_from_py(value)?;
        builder_set!(slf, |b: SnowballBuilder| b.fixed_rate(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_snowball", "SnowballBuilder.gearing")]
    fn gearing<'py>(mut slf: PyRefMut<'py, Self>, value: f64) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: SnowballBuilder| b.gearing(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_snowball", "SnowballBuilder.coupon_floor")]
    fn coupon_floor<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: f64,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: SnowballBuilder| b.coupon_floor(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_snowball", "SnowballBuilder.coupon_cap")]
    fn coupon_cap<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<f64>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        builder_set!(slf, |b: SnowballBuilder| b.coupon_cap_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_snowball", "SnowballBuilder.notional")]
    fn notional<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &PyMoney,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.inner;
        builder_set!(slf, |b: SnowballBuilder| b.notional(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_snowball", "SnowballBuilder.start_date")]
    fn start_date<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = crate::bindings::date_utils::py_to_date(value)?;
        builder_set!(slf, |b: SnowballBuilder| b.start_date(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_snowball", "SnowballBuilder.payment_dates")]
    fn payment_dates<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Vec<Bound<'_, PyAny>>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = {
            let dates = value;
            dates
                .iter()
                .map(crate::bindings::date_utils::py_to_date)
                .collect::<PyResult<Vec<_>>>()?
        };
        builder_set!(slf, |b: SnowballBuilder| b.payment_dates(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_snowball", "SnowballBuilder.forward_curve_id")]
    fn forward_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: SnowballBuilder| b.forward_curve_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_snowball", "SnowballBuilder.index_tenor")]
    fn index_tenor<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "index_tenor")?;
        builder_set!(slf, |b: SnowballBuilder| b.index_tenor(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_snowball", "SnowballBuilder.discount_curve_id")]
    fn discount_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: SnowballBuilder| b.discount_curve_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_snowball", "SnowballBuilder.vol_surface_id")]
    fn vol_surface_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<&str>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.map(Into::into);
        builder_set!(slf, |b: SnowballBuilder| b.vol_surface_id_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_snowball", "SnowballBuilder.call_provision")]
    fn call_provision<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "call_provision")?;
        builder_set!(slf, |b: SnowballBuilder| b.call_provision_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_snowball", "SnowballBuilder.day_count")]
    fn day_count<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = super::convert::day_count_from_py(value, "day_count")?;
        builder_set!(slf, |b: SnowballBuilder| b.day_count(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_snowball", "SnowballBuilder.attributes")]
    fn attributes<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = attributes_from_py(value)?;
        builder_set!(slf, |b: SnowballBuilder| b.attributes(value))
    }
}

/// Register the bond future and its builder.
pub fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PySnowball>()?;
    module.add_class::<PySnowballBuilder>()?;
    Ok(())
}
