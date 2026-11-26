//! An inflation cap or floor with specified coupon and observation terms.

use crate::bindings::core::money::PyMoney;
use crate::bindings::module_utils::py_to_serde;
use crate::bindings::pydoc::pydoc;
use crate::errors::core_to_py;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_valuations::instruments::rates::inflation_cap_floor::{
    InflationCapFloor, InflationCapFloorBuilder,
};
use pyo3::prelude::*;

use super::convert::{attributes_from_py, instrument_expiry};
use super::typed_macros::{builder_set, instrument_envelope_methods, instrument_trait_methods};

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_inflation_cap_floor", "InflationCapFloor")]
#[pyclass(
    name = "InflationCapFloor",
    module = "finstack_quant.valuations.instruments",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyInflationCapFloor {
    pub(crate) inner: InflationCapFloor,
}

instrument_envelope_methods!(
    PyInflationCapFloor(
        "InflationCapFloor",
        module = "finstack_quant.valuations.instruments._typed_inflation_cap_floor"
    ),
    InflationCapFloor,
    builder = PyInflationCapFloorBuilder("InflationCapFloorBuilder", no_fields),
    seed = InflationCapFloor::builder(),
);
instrument_trait_methods!(
    PyInflationCapFloor,
    "InflationCapFloor",
    module = "finstack_quant.valuations.instruments._typed_inflation_cap_floor"
);

#[pymethods]
impl PyInflationCapFloor {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_inflation_cap_floor", "InflationCapFloor.to_builder")]
    fn to_builder(&self) -> PyInflationCapFloorBuilder {
        PyInflationCapFloorBuilder {
            inner: Some(self.inner.to_builder()),
        }
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_inflation_cap_floor", "InflationCapFloor.example")]
    #[staticmethod]
    fn example() -> PyResult<Self> {
        InflationCapFloor::example()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_inflation_cap_floor", "InflationCapFloor.expiry")]
    #[getter]
    fn expiry<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        instrument_expiry(py, &self.inner)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_inflation_cap_floor", "InflationCapFloor.rate_option_type")]
    #[getter]
    fn rate_option_type<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.rate_option_type)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_inflation_cap_floor", "InflationCapFloor.strike")]
    #[getter]
    fn strike<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::core::money::decimal_to_py(py, self.inner.strike)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_inflation_cap_floor", "InflationCapFloor.start_date")]
    #[getter]
    fn start_date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::date_utils::date_to_py(py, self.inner.start_date)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_inflation_cap_floor", "InflationCapFloor.maturity")]
    #[getter]
    fn maturity<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::date_utils::date_to_py(py, self.inner.maturity)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_inflation_cap_floor", "InflationCapFloor.frequency")]
    #[getter]
    fn frequency<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.frequency)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_inflation_cap_floor", "InflationCapFloor.stub")]
    #[getter]
    fn stub<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.stub)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_inflation_cap_floor", "InflationCapFloor.business_day_convention")]
    #[getter]
    fn business_day_convention(&self) -> PyResult<String> {
        super::convert::enum_to_py_string(&self.inner.business_day_convention)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_inflation_cap_floor", "InflationCapFloor.lag")]
    #[getter]
    fn lag<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.lag)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_inflation_cap_floor", "InflationCapFloor.interpolation")]
    #[getter]
    fn interpolation<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.interpolation)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_inflation_cap_floor", "InflationCapFloor.volatility_expiry")]
    #[getter]
    fn volatility_expiry<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.volatility_expiry)
    }
}

crate::bindings::macros::py_getters!(PyInflationCapFloor |s| {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_inflation_cap_floor", "InflationCapFloor.notional")]
    notional -> PyMoney = PyMoney::from_inner(s.inner.notional);
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_inflation_cap_floor", "InflationCapFloor.day_count")]
    day_count -> crate::bindings::core::dates::daycount::PyDayCount = crate::bindings::core::dates::daycount::PyDayCount::from_inner(s.inner.day_count);
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_inflation_cap_floor", "InflationCapFloor.calendar_id")]
    calendar_id -> Option<String> = s.inner.calendar_id.as_ref().map(ToString::to_string);
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_inflation_cap_floor", "InflationCapFloor.inflation_index_id")]
    inflation_index_id -> String = s.inner.inflation_index_id.to_string();
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_inflation_cap_floor", "InflationCapFloor.discount_curve_id")]
    discount_curve_id -> String = s.inner.discount_curve_id.to_string();
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_inflation_cap_floor", "InflationCapFloor.vol_surface_id")]
    vol_surface_id -> String = s.inner.vol_surface_id.to_string();
});

#[doc = pydoc!("finstack_quant.valuations.instruments._typed_inflation_cap_floor", "InflationCapFloorBuilder")]
#[pyclass(
    name = "InflationCapFloorBuilder",
    module = "finstack_quant.valuations.instruments",
    skip_from_py_object
)]
pub struct PyInflationCapFloorBuilder {
    inner: Option<InflationCapFloorBuilder>,
}

#[pymethods]
impl PyInflationCapFloorBuilder {
    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_inflation_cap_floor", "InflationCapFloorBuilder.id")]
    fn id<'py>(mut slf: PyRefMut<'py, Self>, value: &str) -> PyResult<PyRefMut<'py, Self>> {
        let value = InstrumentId::new(value);
        builder_set!(slf, |b: InflationCapFloorBuilder| b.id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_inflation_cap_floor", "InflationCapFloorBuilder.rate_option_type")]
    fn rate_option_type<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "rate_option_type")?;
        builder_set!(slf, |b: InflationCapFloorBuilder| b.rate_option_type(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_inflation_cap_floor", "InflationCapFloorBuilder.notional")]
    fn notional<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &PyMoney,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.inner;
        builder_set!(slf, |b: InflationCapFloorBuilder| b.notional(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_inflation_cap_floor", "InflationCapFloorBuilder.strike")]
    fn strike<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = crate::bindings::core::money::decimal_from_py(value)?;
        builder_set!(slf, |b: InflationCapFloorBuilder| b.strike(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_inflation_cap_floor", "InflationCapFloorBuilder.start_date")]
    fn start_date<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = crate::bindings::date_utils::py_to_date(value)?;
        builder_set!(slf, |b: InflationCapFloorBuilder| b.start_date(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_inflation_cap_floor", "InflationCapFloorBuilder.maturity")]
    fn maturity<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = crate::bindings::date_utils::py_to_date(value)?;
        builder_set!(slf, |b: InflationCapFloorBuilder| b.maturity(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_inflation_cap_floor", "InflationCapFloorBuilder.frequency")]
    fn frequency<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "frequency")?;
        builder_set!(slf, |b: InflationCapFloorBuilder| b.frequency(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_inflation_cap_floor", "InflationCapFloorBuilder.day_count")]
    fn day_count<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = super::convert::day_count_from_py(value, "day_count")?;
        builder_set!(slf, |b: InflationCapFloorBuilder| b.day_count(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_inflation_cap_floor", "InflationCapFloorBuilder.stub")]
    fn stub<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "stub")?;
        builder_set!(slf, |b: InflationCapFloorBuilder| b.stub(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_inflation_cap_floor", "InflationCapFloorBuilder.business_day_convention")]
    fn business_day_convention<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "business_day_convention")?;
        builder_set!(slf, |b: InflationCapFloorBuilder| b
            .business_day_convention(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_inflation_cap_floor", "InflationCapFloorBuilder.calendar_id")]
    fn calendar_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: Option<&str>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.map(Into::into);
        builder_set!(slf, |b: InflationCapFloorBuilder| b.calendar_id_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_inflation_cap_floor", "InflationCapFloorBuilder.inflation_index_id")]
    fn inflation_index_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: InflationCapFloorBuilder| b
            .inflation_index_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_inflation_cap_floor", "InflationCapFloorBuilder.discount_curve_id")]
    fn discount_curve_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: InflationCapFloorBuilder| b
            .discount_curve_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_inflation_cap_floor", "InflationCapFloorBuilder.vol_surface_id")]
    fn vol_surface_id<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = value.into();
        builder_set!(slf, |b: InflationCapFloorBuilder| b.vol_surface_id(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_inflation_cap_floor", "InflationCapFloorBuilder.lag")]
    fn lag<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "lag")?;
        builder_set!(slf, |b: InflationCapFloorBuilder| b.lag_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_inflation_cap_floor", "InflationCapFloorBuilder.interpolation")]
    fn interpolation<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "interpolation")?;
        builder_set!(slf, |b: InflationCapFloorBuilder| b
            .interpolation_opt(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_inflation_cap_floor", "InflationCapFloorBuilder.volatility_expiry")]
    fn volatility_expiry<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = py_to_serde(value.py(), value, "volatility_expiry")?;
        builder_set!(slf, |b: InflationCapFloorBuilder| b
            .volatility_expiry(value))
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._typed_inflation_cap_floor", "InflationCapFloorBuilder.attributes")]
    fn attributes<'py>(
        mut slf: PyRefMut<'py, Self>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = attributes_from_py(value)?;
        builder_set!(slf, |b: InflationCapFloorBuilder| b.attributes(value))
    }
}

/// Register the bond future and its builder.
pub fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyInflationCapFloor>()?;
    module.add_class::<PyInflationCapFloorBuilder>()?;
    Ok(())
}

/// Names this module contributes to `finstack_quant.valuations.instruments.__all__`.
pub(crate) const EXPORTS: &[&str] = &["InflationCapFloor", "InflationCapFloorBuilder"];
